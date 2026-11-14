//! A pinned tab's or favorite's return mark (#2338): while it's away from its pinned URL, Lucide's
//! undo arrow in a round notch cut out of its icon's bottom right, so the row says at rest that a
//! click on its favicon goes back, which before showed only under the pointer.

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui::{AnyElement, App, Image, ImageId, IntoElement, RenderImage, div, img, prelude::*, px};
use image::{Frame, RgbaImage};

use super::{
    SECONDARY, Shell,
    swap::{NONE, Swapped, UNDO},
};
use crate::{
    palette::Palette,
    tokens::shell::{RETURN_GLYPH, RETURN_NOTCH, RETURN_OUT},
};

/// The bitmap an icon draws, as a favicon is handed to the UI or as the UI made it from one.
#[derive(Clone)]
pub(super) enum Source {
    Encoded(Arc<Image>),
    Decoded(Arc<RenderImage>),
}

impl Source {
    pub(super) fn img(&self) -> gpui::Img {
        match self {
            Source::Encoded(image) => img(image.clone()),
            Source::Decoded(image) => img(image.clone()),
        }
    }
}

/// An icon's bitmap and the size it's drawn at, centred in its box, so the notch can be cut out
/// of it once rather than drawn around it each frame.
pub(super) struct Bitmap {
    pub(super) source: Source,
    pub(super) drawn: f32,
}

impl Shell {
    /// `tab`'s icon, which `icon` makes, in a box `side` square, notched with the return mark while
    /// the tab is away from its pinned URL or the mark is coming or going. `bitmap` is what `icon`
    /// draws, if it's a bitmap: at rest the notch is cut out of a copy of it, made once.
    pub(super) fn with_return_mark(
        &self,
        palette: &Palette,
        tab: u64,
        side: f32,
        bitmap: Option<Bitmap>,
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
        let radius = RETURN_NOTCH / 2. * open;
        let at_rest = bitmap
            .filter(|_| open == 1.)
            .and_then(|bitmap| Some((cut_out(&bitmap, side, middle, radius, cx)?, bitmap.drawn)));
        let frame = div().relative().flex_none().size(px(side));
        let frame = match at_rest {
            // One image, as an icon that isn't away is.
            Some((cut, drawn)) => frame
                .flex()
                .items_center()
                .justify_center()
                .child(img(cut).size(px(drawn))),
            // While the notch opens or closes, and for an emoji or the globe, which have no bitmap.
            None => frame.children(notched(
                side,
                middle,
                radius,
                f32::from(self.hairline()),
                &icon,
            )),
        };
        frame
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

/// Which bitmap a cut copy is of.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Encoded(u64),
    Decoded(ImageId),
}

/// `bitmap` with the disc `radius` about (`middle`, `middle`) in its `side` box cut out, made the
/// first time it's asked for and kept, so an away icon at rest costs one image a frame, as an icon
/// that isn't away does. None if it can't be decoded, which draws it in strips.
fn cut_out(
    bitmap: &Bitmap,
    side: f32,
    middle: f32,
    radius: f32,
    cx: &App,
) -> Option<Arc<RenderImage>> {
    // A session sees a few hundred favicons; past CUT_COPIES the lot is dropped and made again as
    // drawn.
    const CUT_COPIES: usize = 512;
    thread_local! {
        static MADE: RefCell<HashMap<(Key, [u32; 4]), Option<Arc<RenderImage>>>> =
            RefCell::default();
    }
    let key = match &bitmap.source {
        Source::Encoded(image) => Key::Encoded(image.id),
        Source::Decoded(image) => Key::Decoded(image.id),
    };
    let shape = [side, bitmap.drawn, middle, radius].map(f32::to_bits);
    MADE.with_borrow_mut(|made| {
        if made.len() >= CUT_COPIES {
            made.clear();
        }
        made.entry((key, shape))
            .or_insert_with(|| {
                let image = match &bitmap.source {
                    // As GPUI decodes it to draw it, SVG and all.
                    Source::Encoded(image) => image.to_image_data(cx.svg_renderer()).ok()?,
                    Source::Decoded(image) => image.clone(),
                };
                let size = image.size(0);
                let (width, height) = (size.width.0 as u32, size.height.0 as u32);
                let mut data = image.as_bytes(0)?.to_vec();
                cut(
                    &mut data,
                    (width, height),
                    bitmap.drawn,
                    side,
                    middle,
                    radius,
                );
                let frame = Frame::new(RgbaImage::from_raw(width, height, data)?);
                Some(Arc::new(RenderImage::new(vec![frame])))
            })
            .clone()
    })
}

/// Clears what of `data`, `width` by `height` BGRA pixels drawn to fit `drawn` square centred in a
/// `side` box, lies in the disc `radius` about (`middle`, `middle`), its edge smoothed over one of
/// the bitmap's pixels.
fn cut(
    data: &mut [u8],
    (width, height): (u32, u32),
    drawn: f32,
    side: f32,
    middle: f32,
    radius: f32,
) {
    // A bitmap that isn't square is drawn whole, to fit, as `img` draws it.
    let pixel = (drawn / width as f32).min(drawn / height as f32);
    let left = (side - width as f32 * pixel) / 2.;
    let top = (side - height as f32 * pixel) / 2.;
    for (index, bgra) in data.chunks_exact_mut(4).enumerate() {
        let (i, j) = (index as u32 % width, index as u32 / width);
        let x = left + (i as f32 + 0.5) * pixel;
        let y = top + (j as f32 + 0.5) * pixel;
        let off = (x - middle).hypot(y - middle) - radius;
        let cover = (off / pixel + 0.5).clamp(0., 1.);
        bgra[3] = (bgra[3] as f32 * cover).round() as u8;
    }
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

    /// The cut copy of a favicon is clear inside the disc and untouched outside it, past a pixel
    /// of smoothing at its edge, for a square bitmap and one drawn to fit.
    #[test]
    fn the_cut_clears_the_disc_and_nothing_else() {
        let (side, drawn, middle, radius) = (18., 16., 15.5, 5.5);
        for (width, height) in [(32, 32), (48, 24)] {
            let mut data = vec![255u8; (width * height * 4) as usize];
            cut(&mut data, (width, height), drawn, side, middle, radius);
            let pixel = (drawn / width as f32).min(drawn / height as f32);
            let (left, top) = (
                (side - width as f32 * pixel) / 2.,
                (side - height as f32 * pixel) / 2.,
            );
            let mut cleared = 0;
            for j in 0..height {
                for i in 0..width {
                    let alpha = data[((j * width + i) * 4 + 3) as usize];
                    let (x, y) = (
                        left + (i as f32 + 0.5) * pixel,
                        top + (j as f32 + 0.5) * pixel,
                    );
                    let off = (x - middle).hypot(y - middle) - radius;
                    if off <= -pixel {
                        assert_eq!(alpha, 0, "inside at {i}, {j} of {width}x{height}");
                        cleared += 1;
                    } else if off >= pixel {
                        assert_eq!(alpha, 255, "outside at {i}, {j} of {width}x{height}");
                    }
                    let colour = &data[((j * width + i) * 4) as usize..][..3];
                    assert_eq!(colour, [255; 3], "the colour is kept");
                }
            }
            assert!(cleared > 0, "{width}x{height} has a notch");
        }
    }

    /// An away icon at rest is cut once: asked again, the same image comes back.
    #[gpui::test]
    fn a_cut_copy_is_made_once(cx: &mut gpui::TestAppContext) {
        let pixels = RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]));
        let image = Arc::new(RenderImage::new(vec![Frame::new(pixels)]));
        let bitmap = Bitmap {
            source: Source::Decoded(image),
            drawn: 16.,
        };
        cx.update(|cx| {
            let first = cut_out(&bitmap, 18., 15.5, 5.5, cx).expect("decoded");
            let again = cut_out(&bitmap, 18., 15.5, 5.5, cx).expect("decoded");
            assert!(Arc::ptr_eq(&first, &again));
            let other = cut_out(&bitmap, 14., 11.5, 5.5, cx).expect("decoded");
            assert!(!Arc::ptr_eq(&first, &other), "another size is another copy");
        });
    }
}
