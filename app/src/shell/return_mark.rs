//! A pinned tab's or favorite's return mark (#2338): while it's away from its pinned URL, Lucide's
//! undo arrow in a round notch cut out of its icon's bottom right, so the row says at rest that a
//! click on its favicon goes back, which before showed only under the pointer.

use gpui::{AnyElement, App, IntoElement, div, prelude::*, px};

use super::{
    SECONDARY, Shell,
    swap::{NONE, Swapped, UNDO},
};
use crate::{
    palette::Palette,
    tokens::shell::{RETURN_GLYPH, RETURN_NOTCH, RETURN_OUT},
};

/// How tall each strip of the icon beside the notch is: a device pixel at 2x, and half of one at
/// 1x, where each pixel's middle still falls in one strip, so the strips meet with no seam.
const STRIP: f32 = 0.5;

impl Shell {
    /// `tab`'s icon, which `icon` makes, in a box `side` square, notched with the return mark while
    /// the tab is away from its pinned URL or the mark is coming or going.
    pub(super) fn with_return_mark(
        &self,
        palette: &Palette,
        tab: u64,
        side: f32,
        icon: impl Fn() -> AnyElement,
        cx: &App,
    ) -> AnyElement {
        // Proof only (#2338, not for merge): today's look, for the befores.
        if std::env::var_os("PREVIEW_NO_RETURN_MARK").is_some() {
            return icon();
        }
        let away = self.page(tab).away_from_pinned_url;
        let what = Swapped::Away(tab);
        // The notch opens and closes with its glyph's change, and is whole under Reduce Motion,
        // as the glyph is at its size.
        let open = match self.swaps.toward(what, UNDO) {
            Some(way) => crate::motion::scale(way, cx),
            None if away => 1.,
            None => return icon(),
        };
        let middle = side + RETURN_OUT - RETURN_NOTCH / 2.;
        let glyph = self.swaps.glyph(
            what,
            if away { UNDO } else { NONE },
            RETURN_GLYPH,
            palette.text(SECONDARY),
            cx,
        );
        div()
            .relative()
            .flex_none()
            .size(px(side))
            .children(notched(side, middle, RETURN_NOTCH / 2. * open, &icon))
            .child(
                div()
                    .absolute()
                    .left(px(middle - RETURN_NOTCH / 2.))
                    .top(px(middle - RETURN_NOTCH / 2.))
                    .size(px(RETURN_NOTCH))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph),
            )
            .into_any_element()
    }
}

impl Shell {
    /// Proof only (#2338, not for merge): `tabs`' marks coming, or going with the tabs back.
    pub fn mock_return_motion(&mut self, tabs: &[u64], come: bool) {
        for &tab in tabs {
            if !come && let Some(each) = self.tabs.get_mut(&tab) {
                each.page.away_from_pinned_url = false;
            }
            let (from, to) = if come { (NONE, UNDO) } else { (UNDO, NONE) };
            self.swaps
                .start(Swapped::Away(tab), from, to, std::time::Instant::now());
        }
    }
}

/// `icon`, `side` square, but for a disc `radius` about (`middle`, `middle`): whole above the
/// disc, and beside it in strips each clipped at the disc's edge. GPUI can't cut one shape out of
/// another, and a disc painted over the icon would show as a patch on the Mac's translucent window,
/// so the icon is drawn around the notch and what's under it shows through as it is.
fn notched(side: f32, middle: f32, radius: f32, icon: &dyn Fn() -> AnyElement) -> Vec<AnyElement> {
    let part = |top: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left_0()
            .top(px(top))
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(-top))
                    .size(px(side))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon()),
            )
            .into_any_element()
    };
    strips(side, middle, radius)
        .into_iter()
        .map(|(top, width, height)| part(top, width, height))
        .collect()
}

/// The rectangles `notched` draws the icon in, each its top, width and height: down to the disc
/// whole, then strips each stopping where the disc is widest within it.
fn strips(side: f32, middle: f32, radius: f32) -> Vec<(f32, f32, f32)> {
    if radius <= 0. {
        return vec![(0., side, side)];
    }
    let start = (((middle - radius) / STRIP).floor() * STRIP).clamp(0., side);
    let mut rects = vec![(0., side, start)];
    let mut top = start;
    while top < side {
        let bottom = (top + STRIP).min(side);
        let nearest = middle.clamp(top, bottom) - middle;
        let reach = (radius * radius - nearest * nearest).max(0.).sqrt();
        rects.push((top, (middle - reach).clamp(0., side), bottom - top));
        top = bottom;
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The strips cover the icon with no gap or overlap, leave out the disc and only the disc,
    /// and a closed notch leaves the icon whole.
    #[test]
    fn the_notch_leaves_out_the_disc_and_nothing_else() {
        let (side, middle, radius) = (18., 15.5, 5.5);
        let rects = strips(side, middle, radius);
        let mut top = 0.;
        for &(y, _, height) in &rects {
            assert_eq!(y, top, "the strips meet");
            top += height;
        }
        assert_eq!(top, side, "the strips reach the bottom");
        let inside = |x: f32, y: f32| {
            rects
                .iter()
                .any(|&(ry, w, h)| y >= ry && y < ry + h && x < w)
        };
        for i in 0..36 {
            for j in 0..36 {
                let (x, y) = (i as f32 * 0.5 + 0.25, j as f32 * 0.5 + 0.25);
                let in_disc = (x - middle).hypot(y - middle) < radius;
                // A pixel's middle at the disc's rim may go either way within a strip.
                let rim = ((x - middle).hypot(y - middle) - radius).abs() < STRIP;
                if !rim {
                    assert_eq!(inside(x, y), !in_disc, "at {x}, {y}");
                }
            }
        }
        assert_eq!(
            strips(side, middle, 0.),
            [(0., side, side)],
            "a closed notch cuts nothing"
        );
    }
}
