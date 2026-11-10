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
            .children(notched(
                side,
                middle,
                RETURN_NOTCH / 2. * open,
                f32::from(self.hairline()),
                &icon,
            ))
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

/// `icon`, `side` square, but for a disc `radius` about (`middle`, `middle`): whole above the
/// disc, and beside it in strips each clipped at the disc's edge. GPUI can't cut one shape out of
/// another, and a disc painted over the icon would show as a patch on the Mac's translucent window,
/// so the icon is drawn around the notch and what's under it shows through as it is.
fn notched(
    side: f32,
    middle: f32,
    radius: f32,
    pixel: f32,
    icon: &dyn Fn() -> AnyElement,
) -> Vec<AnyElement> {
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
    strips(side, middle, radius, pixel)
        .into_iter()
        .map(|(top, width, height)| part(top, width, height))
        .collect()
}

/// The rectangles `notched` draws the icon in, each its top, width and height: down to the disc
/// whole, then strips each stopping where the disc is widest within it. Each strip is a device
/// pixel, `pixel` points, tall: GPUI rounds a box's height to whole device pixels before layout, so
/// a thinner one has none and draws nothing.
fn strips(side: f32, middle: f32, radius: f32, pixel: f32) -> Vec<(f32, f32, f32)> {
    if radius <= 0. {
        return vec![(0., side, side)];
    }
    let start = (((middle - radius) / pixel).floor() * pixel).clamp(0., side);
    let mut rects = vec![(0., side, start)];
    let mut top = start;
    while top < side {
        let bottom = (top + pixel).min(side);
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

    /// At 1x and 2x, the strips cover the icon with no gap or overlap, each a whole device pixel
    /// tall, leave out the disc and only the disc, and a closed notch leaves the icon whole.
    #[test]
    fn the_notch_leaves_out_the_disc_and_nothing_else() {
        let (side, middle, radius) = (18., 15.5, 5.5);
        for scale in [1., 2.] {
            let pixel = 1. / scale;
            let rects = strips(side, middle, radius, pixel);
            let mut top = 0.;
            for &(y, _, height) in &rects {
                assert_eq!(y, top, "the strips meet");
                assert_eq!(
                    y * scale,
                    (y * scale).round(),
                    "a strip starts on a device pixel"
                );
                assert!(height >= pixel, "a strip is at least a device pixel tall");
                top += height;
            }
            assert_eq!(top, side, "the strips reach the bottom");
            let inside = |x: f32, y: f32| {
                rects
                    .iter()
                    .any(|&(ry, w, h)| y >= ry && y < ry + h && x < w)
            };
            // Each device pixel's middle.
            let across = (side * scale) as usize;
            for i in 0..across {
                for j in 0..across {
                    let (x, y) = ((i as f32 + 0.5) * pixel, (j as f32 + 0.5) * pixel);
                    let off = (x - middle).hypot(y - middle) - radius;
                    // A pixel at the disc's rim may go either way within its strip.
                    if off.abs() >= pixel {
                        assert_eq!(inside(x, y), off >= 0., "at {x}, {y}, {scale}x");
                    }
                }
            }
            assert_eq!(
                strips(side, middle, 0., pixel),
                [(0., side, side)],
                "a closed notch cuts nothing"
            );
        }
    }
}
