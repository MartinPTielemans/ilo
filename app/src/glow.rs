//! A Space's colour as a glow from the sidebar's foot (#4089): an ellipse of the colour centred on
//! the window's bottom left corner, and a faint one of its second colour on the top right, each
//! fading in a straight line from its centre to nothing part of the way to its edge, as CSS's
//! `radial-gradient(… at 0% 100%, color, transparent 72%)` draws it.
//!
//! GPUI draws no radial gradient, so each is a picture of its colour, a quarter of the ellipse
//! from the corner out, made once for each colour at a quarter of its size, as the falloff is
//! smooth and the sampler is linear, and kept: a frame that changes nothing makes none and draws
//! two sprites, and an idle window draws no frame. The picker changes a colour as it's dragged,
//! which makes one each frame and drops the oldest from the atlas, so dragging doesn't grow it.

use std::{cell::RefCell, sync::Arc};

use gpui::{App, Bounds, Pixels, RenderImage, Rgba, canvas, div, point, prelude::*, px, size};
use image::{Frame, RgbaImage};

use crate::tokens::palette::{
    FAR_GLOW_FADE, FAR_GLOW_HEIGHT, FAR_GLOW_WIDTH, GLOW_FADE, GLOW_HEIGHT, GLOW_WIDTH,
};

/// Which corner a glow is centred on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// The sidebar's foot, the window's bottom left.
    Foot,
    /// The window's top right.
    Far,
}

impl Corner {
    /// The ellipse's radii, and how far along them it fades to nothing.
    fn shape(self) -> (f32, f32, f32) {
        match self {
            Corner::Foot => (GLOW_WIDTH, GLOW_HEIGHT, GLOW_FADE),
            Corner::Far => (FAR_GLOW_WIDTH, FAR_GLOW_HEIGHT, FAR_GLOW_FADE),
        }
    }

    /// The box the glow shows in, from its corner to where it's gone, in points.
    pub fn extent(self) -> (f32, f32) {
        let (width, height, fade) = self.shape();
        (width * fade, height * fade)
    }

    /// Whether the glow's corner is at the window's left: the foot's, unless the sidebar is on the
    /// right, where the foot is, so the window is `mirrored`.
    fn at_left(self, mirrored: bool) -> bool {
        (self == Corner::Foot) != mirrored
    }

    /// How strong the glow is, 0 to 1, `dx` and `dy` points from its corner.
    pub fn strength(self, dx: f32, dy: f32) -> f32 {
        let (width, height) = self.extent();
        let reach = ((dx / width).powi(2) + (dy / height).powi(2)).sqrt();
        (1. - reach).max(0.)
    }
}

/// The picture's pixels to a point of the glow's box.
const DETAIL: f32 = 0.25;
/// How many colours' pictures are kept: the two glows of the two Spaces a swipe shows, with room.
const KEPT: usize = 8;

thread_local! {
    static PICTURES: RefCell<Vec<((u32, bool, bool), Arc<RenderImage>)>> = const { RefCell::new(Vec::new()) };
}

/// How strong the foot's glow and the far one are at `x`, `y` points from the top left of a
/// window `width` by `height`, `mirrored` with the sidebar on its right.
pub fn strengths(x: f32, y: f32, width: f32, height: f32, mirrored: bool) -> (f32, f32) {
    let from = |corner: Corner| match corner.at_left(mirrored) {
        true => x,
        false => width - x,
    };
    (
        Corner::Foot.strength(from(Corner::Foot), height - y),
        Corner::Far.strength(from(Corner::Far), y),
    )
}

/// `color`'s glow from `corner`, at `opacity`, `mirrored` with the sidebar on the window's right:
/// absolutely placed in the window, which clips what of it a small window has no room for, over
/// the window's ground and under everything else.
pub fn glow(corner: Corner, mirrored: bool, color: Rgba, opacity: f32) -> gpui::Div {
    glow_scaled(corner, mirrored, color, opacity, 1.)
}

/// The glow `scale` times its size, for a small picture of a window, as first run's loops
/// draw one: the same picture, so the same kept colour serves both.
pub fn glow_scaled(
    corner: Corner,
    mirrored: bool,
    color: Rgba,
    opacity: f32,
    scale: f32,
) -> gpui::Div {
    let (width, height) = corner.extent();
    let (width, height) = (width * scale, height * scale);
    let layer = div().absolute().w(px(width)).h(px(height)).opacity(opacity);
    let layer = match corner.at_left(mirrored) {
        true => layer.left_0(),
        false => layer.right_0(),
    };
    let layer = match corner {
        Corner::Foot => layer.bottom_0(),
        Corner::Far => layer.top_0(),
    };
    layer.child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let picture = picture(corner, mirrored, color, window, cx);
                // The whole picture a texel past the box on each side, shown only in the box.
                let whole = picture_bounds(corner, bounds);
                window
                    .paint_image(bounds, whole, px(0.).into(), picture, 0, false)
                    .ok();
            },
        )
        .size_full(),
    )
}

/// The picture of `color`'s glow from `corner`, made once and kept while it's among the last few
/// drawn.
fn picture(
    corner: Corner,
    mirrored: bool,
    color: Rgba,
    window: &mut gpui::Window,
    cx: &mut App,
) -> Arc<RenderImage> {
    let key = (
        crate::theme::packed(color),
        corner.at_left(mirrored),
        corner == Corner::Foot,
    );
    let (found, dropped) = PICTURES.with(|pictures| {
        let mut pictures = pictures.borrow_mut();
        if let Some(at) = pictures.iter().position(|(each, _)| *each == key) {
            let kept = pictures.remove(at);
            let picture = kept.1.clone();
            pictures.push(kept);
            return (picture, None);
        }
        let dropped = (pictures.len() == KEPT).then(|| pictures.remove(0).1);
        let picture = make(corner, mirrored, color);
        pictures.push((key, picture.clone()));
        (picture, dropped)
    });
    if let Some(dropped) = dropped {
        cx.drop_image(dropped, Some(window));
    }
    found
}

/// The glow's texels across and down its box, at `DETAIL`, without the picture's clear border.
fn texels(corner: Corner) -> (u32, u32) {
    let (width, height) = corner.extent();
    (
        (width * DETAIL).ceil().max(1.) as u32,
        (height * DETAIL).ceil().max(1.) as u32,
    )
}

/// Where the whole picture, its clear border with it, is drawn for a glow in `bounds`: a texel
/// further out on each side, so the box shows exactly the glow's texels. GPUI keeps every picture
/// in one atlas with no room between them and samples it linearly at four pixels a texel, so a
/// box drawn to the picture's own edge took its last two pixels half from the picture packed
/// beside it, a bright line at the foot's edge (#4093); here that edge samples the clear border.
fn picture_bounds(corner: Corner, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let (columns, rows) = texels(corner);
    let across = bounds.size.width / columns as f32;
    let down = bounds.size.height / rows as f32;
    Bounds {
        origin: point(bounds.origin.x - across, bounds.origin.y - down),
        size: size(
            bounds.size.width + across * 2.,
            bounds.size.height + down * 2.,
        ),
    }
}

/// The picture: `color` everywhere, its alpha the glow's strength, in BGRA as GPUI keeps images,
/// with a clear texel all round it (`picture_bounds`).
fn make(corner: Corner, mirrored: bool, color: Rgba) -> Arc<RenderImage> {
    let (width, height) = corner.extent();
    let (columns, rows) = texels(corner);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    let (r, g, b) = (channel(color.r), channel(color.g), channel(color.b));
    let mut pixels = Vec::with_capacity(((columns + 2) * (rows + 2) * 4) as usize);
    let clear = [b, g, r, 0];
    pixels.extend(clear.repeat(columns as usize + 2));
    for row in 0..rows {
        pixels.extend(clear);
        for column in 0..columns {
            // Each pixel's middle, in points from the glow's corner.
            let x = (column as f32 + 0.5) / columns as f32 * width;
            let y = (row as f32 + 0.5) / rows as f32 * height;
            let dx = match corner.at_left(mirrored) {
                true => x,
                false => width - x,
            };
            let dy = match corner {
                Corner::Foot => height - y,
                Corner::Far => y,
            };
            pixels.extend([b, g, r, channel(corner.strength(dx, dy))]);
        }
        pixels.extend(clear);
    }
    pixels.extend(clear.repeat(columns as usize + 2));
    let picture = RgbaImage::from_raw(columns + 2, rows + 2, pixels).expect("sized to fit");
    Arc::new(RenderImage::new(vec![Frame::new(picture)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glow_is_strongest_at_its_corner_and_gone_at_its_fade() {
        for corner in [Corner::Foot, Corner::Far] {
            let (width, height) = corner.extent();
            assert_eq!(corner.strength(0., 0.), 1.);
            assert_eq!(corner.strength(width, 0.), 0.);
            assert_eq!(corner.strength(0., height), 0.);
            let halfway = corner.strength(width / 2., 0.);
            assert!((halfway - 0.5).abs() < 1e-4, "{halfway}");
            assert!(
                corner.strength(width / 4., height / 4.) > corner.strength(width / 2., height / 2.)
            );
        }
    }

    /// The picture's outer texels are clear all round, so a glow drawn from the sprite atlas
    /// samples them at its box's edge rather than the next picture packed beside it, whose
    /// brightest column showed as a 2 px line at the foot glow's edge, x 403 (#4093).
    #[test]
    fn a_pictures_edge_is_clear_all_round() {
        let red = Rgba {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 1.,
        };
        for corner in [Corner::Foot, Corner::Far] {
            for mirrored in [false, true] {
                let picture = make(corner, mirrored, red);
                let size = picture.size(0);
                let (columns, rows) = (size.width.0 as usize, size.height.0 as usize);
                let bytes = picture.as_bytes(0).expect("one frame");
                let alpha = |column: usize, row: usize| bytes[(row * columns + column) * 4 + 3];
                for row in 0..rows {
                    for column in [0, columns - 1] {
                        assert_eq!(
                            alpha(column, row),
                            0,
                            "{corner:?} {mirrored}: column {column}, row {row}"
                        );
                    }
                }
                for column in 0..columns {
                    for row in [0, rows - 1] {
                        assert_eq!(
                            alpha(column, row),
                            0,
                            "{corner:?} {mirrored}: column {column}, row {row}"
                        );
                    }
                }
            }
        }
    }

    /// Drawn a texel past its box on each side, the picture's glow texels fill the box exactly.
    #[test]
    fn the_glows_texels_fill_its_box() {
        for corner in [Corner::Foot, Corner::Far] {
            let (width, height) = corner.extent();
            let (columns, rows) = texels(corner);
            for scale in [1., 0.4] {
                let bounds = Bounds {
                    origin: point(px(12.), px(30.)),
                    size: size(px(width * scale), px(height * scale)),
                };
                let whole = picture_bounds(corner, bounds);
                let across = whole.size.width / (columns + 2) as f32;
                let down = whole.size.height / (rows + 2) as f32;
                let near = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() < 1e-3;
                assert!(
                    near(whole.origin.x + across, bounds.origin.x),
                    "{corner:?} {scale}"
                );
                assert!(
                    near(whole.origin.y + down, bounds.origin.y),
                    "{corner:?} {scale}"
                );
                assert!(
                    near(whole.size.width - across * 2., bounds.size.width),
                    "{corner:?} {scale}"
                );
                assert!(
                    near(whole.size.height - down * 2., bounds.size.height),
                    "{corner:?} {scale}"
                );
            }
        }
    }

    #[test]
    fn the_foots_glow_reaches_further_than_the_far_one() {
        let (foot, far) = (Corner::Foot.extent(), Corner::Far.extent());
        assert!(foot.0 > far.0 && foot.1 > far.1);
    }
}
