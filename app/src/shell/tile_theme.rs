//! A selected favorite's tile, themed by its icon as Dia 1.50.1 does in dark mode
//! (docs/ux-spec.md › Favorites). GPUI has no blur, colour filter or blend mode, so the blurred
//! and adjusted images are made here, once for each favicon, and drawn as images.

use std::sync::Arc;

use gpui::{Bounds, Corners, Image, RenderImage, Window, canvas, div, point, prelude::*, px, size};
use image::{Frame, RgbaImage};

use super::{TILE_HEIGHT, TILE_RADIUS};

/// The icon is looked at, and blurred, at no more than this on its long side.
const MAX_SIDE: usize = 32;
use crate::tokens::shell::tile_theme::BLACK;
use crate::tokens::shell::tile_theme::BLUR_OPACITY;
use crate::tokens::shell::tile_theme::BLUR_SCALE;
use crate::tokens::shell::tile_theme::BLUR_SIGMA;
use crate::tokens::shell::tile_theme::COLOURFUL;
use crate::tokens::shell::tile_theme::NEAR_WHITE;
use crate::tokens::shell::tile_theme::NEAR_WHITE_RING;
use crate::tokens::shell::tile_theme::OPAQUE;
use crate::tokens::shell::tile_theme::RING_BAKED_WIDTH;
use crate::tokens::shell::tile_theme::RING_BRIGHTNESS;
use crate::tokens::shell::tile_theme::RING_OPACITY;
use crate::tokens::shell::tile_theme::RING_SATURATION;
use crate::tokens::shell::tile_theme::RING_SCALE;
pub(super) use crate::tokens::shell::tile_theme::RING_WIDTH;

/// How an icon themes its tile.
#[derive(Debug, PartialEq)]
enum Look {
    Colourful,
    /// The mean colour, 0…1.
    OneColour([f32; 3]),
    NearWhite,
}

/// An icon's pixels as straight RGBA, 0…1.
struct Icon {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Icon {
    /// `image`'s pixels, shrunk to MAX_SIDE on the long side if they're bigger.
    fn new(image: &RgbaImage) -> Icon {
        let (width, height) = (image.width() as usize, image.height() as usize);
        let pixels = image
            .pixels()
            .map(|pixel| pixel.0.map(|c| c as f32 / 255.))
            .collect();
        Icon {
            width,
            height,
            pixels,
        }
        .fitted()
    }

    fn fitted(self) -> Icon {
        let long = self.width.max(self.height);
        if long <= MAX_SIDE {
            return self;
        }
        let scale = MAX_SIDE as f32 / long as f32;
        let width = ((self.width as f32 * scale).round() as usize).max(1);
        let height = ((self.height as f32 * scale).round() as usize).max(1);
        let (across, down) = (
            self.width as f32 / width as f32,
            self.height as f32 / height as f32,
        );
        // The mean of the source pixels under each new one, weighted by how much of each it covers
        // and by alpha, so transparent pixels' colours don't bleed in.
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let (mut sum, mut area) = ([0.; 4], 0.);
                let (from_x, to_x) = (x as f32 * across, (x + 1) as f32 * across);
                let (from_y, to_y) = (y as f32 * down, (y + 1) as f32 * down);
                for sy in from_y.floor() as usize..(to_y.ceil() as usize).min(self.height) {
                    for sx in from_x.floor() as usize..(to_x.ceil() as usize).min(self.width) {
                        let covered = ((sx + 1) as f32).min(to_x) - (sx as f32).max(from_x);
                        let covered =
                            covered * (((sy + 1) as f32).min(to_y) - (sy as f32).max(from_y));
                        let [r, g, b, a] = self.pixels[sy * self.width + sx];
                        for (sum, c) in sum.iter_mut().zip([r * a, g * a, b * a, a]) {
                            *sum += c * covered;
                        }
                        area += covered;
                    }
                }
                let alpha = sum[3] / area;
                let colour = |c: f32| if sum[3] > 0. { c / sum[3] } else { 0. };
                pixels.push([colour(sum[0]), colour(sum[1]), colour(sum[2]), alpha]);
            }
        }
        Icon {
            width,
            height,
            pixels,
        }
    }

    /// The mean colour of the pixels that are nearly opaque, and their mean distance from it.
    fn colour(&self) -> Option<([f32; 3], f32)> {
        let opaque: Vec<[f32; 3]> = self
            .pixels
            .iter()
            .filter(|pixel| pixel[3] >= OPAQUE)
            .map(|&[r, g, b, _]| [r, g, b])
            .collect();
        if opaque.is_empty() {
            return None;
        }
        let count = opaque.len() as f32;
        let mean: [f32; 3] =
            std::array::from_fn(|i| opaque.iter().map(|c| c[i]).sum::<f32>() / count);
        let distance = opaque
            .iter()
            .map(|c| (0..3).map(|i| (c[i] - mean[i]).powi(2)).sum::<f32>().sqrt())
            .sum::<f32>()
            / count;
        Some((mean, distance))
    }

    /// None for an icon with no opaque pixel, which themes nothing.
    fn look(&self) -> Option<Look> {
        let (mean, distance) = self.colour()?;
        Some(if distance >= COLOURFUL {
            Look::Colourful
        } else if luminance(mean) > NEAR_WHITE {
            Look::NearWhite
        } else {
            Look::OneColour(mean)
        })
    }
}

/// WCAG's relative luminance of an sRGB colour.
fn luminance([r, g, b]: [f32; 3]) -> f32 {
    let linear = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// White in soft-light over `backdrop`: W3C's formula with a source of 1.
fn soft_light_white(backdrop: f32) -> f32 {
    if backdrop <= 0.25 {
        ((16. * backdrop - 12.) * backdrop + 4.) * backdrop
    } else {
        backdrop.sqrt()
    }
}

/// The icon blurred with a Gaussian of sigma BLUR_SIGMA, cropped to the icon's own extent, as
/// premultiplied RGBA.
struct Blurred {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Blurred {
    fn new(icon: &Icon) -> Blurred {
        let reach = (3. * BLUR_SIGMA).ceil() as isize;
        let mut kernel: Vec<f32> = (-reach..=reach)
            .map(|i| (-((i * i) as f32) / (2. * BLUR_SIGMA * BLUR_SIGMA)).exp())
            .collect();
        let total: f32 = kernel.iter().sum();
        kernel.iter_mut().for_each(|weight| *weight /= total);
        let (width, height) = (icon.width, icon.height);
        let pass = |from: &[[f32; 4]], across: bool| -> Vec<[f32; 4]> {
            (0..width * height)
                .map(|at| {
                    let (x, y) = ((at % width) as isize, (at / width) as isize);
                    let mut sum = [0.; 4];
                    for (offset, weight) in (-reach..=reach).zip(&kernel) {
                        let (x, y) = if across {
                            (x + offset, y)
                        } else {
                            (x, y + offset)
                        };
                        // Beyond the icon is transparent.
                        if (0..width as isize).contains(&x) && (0..height as isize).contains(&y) {
                            let pixel = from[y as usize * width + x as usize];
                            sum.iter_mut()
                                .zip(pixel)
                                .for_each(|(sum, c)| *sum += c * weight);
                        }
                    }
                    sum
                })
                .collect()
        };
        let premultiplied: Vec<[f32; 4]> = icon
            .pixels
            .iter()
            .map(|&[r, g, b, a]| [r * a, g * a, b * a, a])
            .collect();
        let pixels = pass(&pass(&premultiplied, true), false);
        Blurred {
            width,
            height,
            pixels,
        }
    }

    /// The premultiplied pixel at `u`, `v` in 0…1 across the image, bilinear.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (x, y) = (u * self.width as f32 - 0.5, v * self.height as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let at = |x: f32, y: f32| {
            let x = (x as isize).clamp(0, self.width as isize - 1) as usize;
            let y = (y as isize).clamp(0, self.height as isize - 1) as usize;
            self.pixels[y * self.width + x]
        };
        let (a, b, c, d) = (
            at(x0, y0),
            at(x0 + 1., y0),
            at(x0, y0 + 1.),
            at(x0 + 1., y0 + 1.),
        );
        std::array::from_fn(|i| {
            (a[i] * (1. - tx) + b[i] * tx) * (1. - ty) + (c[i] * (1. - tx) + d[i] * tx) * ty
        })
    }

    /// The image the tile shows under its icon, which is drawn at BLUR_OPACITY.
    fn layer(&self) -> Arc<RenderImage> {
        let mut data = Vec::with_capacity(self.pixels.len() * 4);
        for &[r, g, b, a] in &self.pixels {
            let straight = |c: f32| {
                if a > 0. {
                    (c / a * 255.).round() as u8
                } else {
                    0
                }
            };
            data.extend([
                straight(b),
                straight(g),
                straight(r),
                (a * 255.).round() as u8,
            ]);
        }
        render_image(self.width, self.height, data)
    }

    /// The ring of a tile BAKED_WIDTH wide: the blurred image over black at RING_OPACITY with its
    /// saturation and brightness changed, cut to the band RING_WIDTH wide inside the tile's edge.
    fn ring(&self) -> Arc<RenderImage> {
        let tile = (RING_BAKED_WIDTH, TILE_HEIGHT);
        let cover = filling((self.width, self.height), tile);
        let (width, height) = (
            (tile.0 * RING_SCALE) as usize,
            (tile.1 * RING_SCALE) as usize,
        );
        let mut data = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let (px, py) = ((x as f32 + 0.5) / RING_SCALE, (y as f32 + 0.5) / RING_SCALE);
                let outside = rounded_distance(px, py, tile, 0., TILE_RADIUS);
                let inside = rounded_distance(px, py, tile, RING_WIDTH, TILE_RADIUS - RING_WIDTH);
                // Pixels' coverage of the band, from their distance to its edges.
                let band = (0.5 - outside * RING_SCALE).clamp(0., 1.)
                    * (0.5 + inside * RING_SCALE).clamp(0., 1.);
                // Most of the tile is off the band, where the colour never shows: skipping it
                // keeps the theme, made on the frame that first shows the favorite, cheap.
                if band == 0. {
                    data.extend([0; 4]);
                    continue;
                }
                let u = 0.5 + (px - tile.0 / 2.) / cover.0;
                let v = 0.5 + (py - tile.1 / 2.) / cover.1;
                let [r, g, b, a] = self.sample(u, v);
                let colour = if a > 0. {
                    adjusted([r / a, g / a, b / a])
                } else {
                    [0.; 3]
                };
                let shown = RING_OPACITY * a;
                let byte = |c: f32| (c * shown * 255.).round() as u8;
                data.extend([
                    byte(colour[2]),
                    byte(colour[1]),
                    byte(colour[0]),
                    (band * 255.).round() as u8,
                ]);
            }
        }
        render_image(width, height, data)
    }
}

/// The size, in points, at which an image of `image` pixels aspect-fills BLUR_SCALE times a tile
/// of `tile`.
fn filling((width, height): (usize, usize), (tile_width, tile_height): (f32, f32)) -> (f32, f32) {
    let fill = (tile_width / width as f32).max(tile_height / height as f32) * BLUR_SCALE;
    (width as f32 * fill, height as f32 * fill)
}

/// How far (px, py) is outside the rounded rectangle `inset` in from `tile`'s edges, with corners
/// of `radius`; negative inside.
fn rounded_distance(px: f32, py: f32, tile: (f32, f32), inset: f32, radius: f32) -> f32 {
    let (half_width, half_height) = (tile.0 / 2., tile.1 / 2.);
    let q = [
        (px - half_width).abs() - (half_width - inset - radius),
        (py - half_height).abs() - (half_height - inset - radius),
    ];
    q[0].max(0.).hypot(q[1].max(0.)) + q[0].max(q[1]).min(0.) - radius
}

/// CIColorControls' saturation, then brightness.
fn adjusted(colour: [f32; 3]) -> [f32; 3] {
    let grey = 0.2125 * colour[0] + 0.7154 * colour[1] + 0.0721 * colour[2];
    colour.map(|c| (grey + (c - grey) * RING_SATURATION + RING_BRIGHTNESS).clamp(0., 1.))
}

/// An image from straight BGRA bytes, the order GPUI draws.
fn render_image(width: usize, height: usize, data: Vec<u8>) -> Arc<RenderImage> {
    let image = RgbaImage::from_raw(width as u32, height as u32, data).expect("sized to fit");
    Arc::new(RenderImage::new(vec![Frame::new(image)]))
}

/// How a tile looks while its favorite is shown.
pub(super) enum TileTheme {
    /// A colourful icon: the tile is white 0.20 with the blurred icon over it, and a ring of the
    /// same icon.
    Blur {
        blurred: Arc<RenderImage>,
        blurred_size: (usize, usize),
        ring: Arc<RenderImage>,
    },
    /// A one-colour icon: the tile is filled with `fill` (0xRRGGBBAA), the icon is `template`, a
    /// white copy of it, and the ring is `ring`.
    Flat {
        fill: u32,
        ring: u32,
        template: Arc<RenderImage>,
    },
}

impl TileTheme {
    /// What `favicon` themes a tile with, if it has opaque pixels to go by.
    pub(super) fn new(favicon: &Image) -> Option<TileTheme> {
        let decoded = image::load_from_memory(&favicon.bytes).ok()?.into_rgba8();
        let icon = Icon::new(&decoded);
        Some(match icon.look()? {
            Look::Colourful => {
                let blurred = Blurred::new(&icon);
                TileTheme::Blur {
                    blurred_size: (blurred.width, blurred.height),
                    blurred: blurred.layer(),
                    ring: blurred.ring(),
                }
            }
            look => {
                let (fill, ring) = match look {
                    Look::OneColour(mean) => {
                        let byte = |c: f32| (c * 255.).round() as u32;
                        let pack = |c: [f32; 3]| {
                            (byte(c[0]) << 24) | (byte(c[1]) << 16) | (byte(c[2]) << 8) | 0xff
                        };
                        (pack(mean), pack(mean.map(soft_light_white)))
                    }
                    _ => (BLACK, NEAR_WHITE_RING),
                };
                let mut data = Vec::with_capacity(decoded.len());
                for pixel in decoded.pixels() {
                    data.extend([255, 255, 255, pixel.0[3]]);
                }
                let template =
                    render_image(decoded.width() as usize, decoded.height() as usize, data);
                TileTheme::Flat {
                    fill,
                    ring,
                    template,
                }
            }
        })
    }

    /// What goes over the tile's fill: the blurred icon and the ring, for a colourful icon.
    pub(super) fn layers(&self) -> Option<gpui::Div> {
        let TileTheme::Blur {
            blurred,
            blurred_size,
            ring,
        } = self
        else {
            return None;
        };
        let (blurred, blurred_size, ring) = (blurred.clone(), *blurred_size, ring.clone());
        let blur = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| paint_blur(window, bounds, &blurred, blurred_size),
        );
        let ring = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| paint_ring(window, bounds, &ring),
        );
        Some(
            div()
                .absolute()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .opacity(BLUR_OPACITY)
                        .child(blur.absolute().size_full()),
                )
                .child(ring.absolute().size_full()),
        )
    }
}

/// Draws the blurred icon aspect-filled to BLUR_SCALE tiles, centred on the tile at `bounds`.
fn paint_blur(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    image: &Arc<RenderImage>,
    (w, h): (usize, usize),
) {
    let tile = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let (cover_width, cover_height) = filling((w, h), tile);
    let cover = size(px(cover_width), px(cover_height));
    let radii = Corners::all(px(TILE_RADIUS));
    window
        .paint_image(
            bounds,
            Bounds::centered_at(bounds.center(), cover),
            radii,
            image.clone(),
            0,
            false,
        )
        .ok();
}

/// Draws the ring's image in nine parts' worth: the corners as they were made, and the top and
/// bottom bands stretched between them, so one image fits every width of tile.
fn paint_ring(window: &mut Window, bounds: Bounds<gpui::Pixels>, image: &Arc<RenderImage>) {
    let (left, top) = (bounds.origin.x, bounds.origin.y);
    let (width, height) = (bounds.size.width, bounds.size.height);
    let (baked, cap, band) = (px(RING_BAKED_WIDTH), px(TILE_RADIUS), px(RING_WIDTH));
    let mut paint = |part: Bounds<gpui::Pixels>, whole: Bounds<gpui::Pixels>| {
        window
            .paint_image(part, whole, Corners::default(), image.clone(), 0, false)
            .ok();
    };
    let at = |x, y, w, h| Bounds::new(point(x, y), size(w, h));
    paint(at(left, top, cap, height), at(left, top, baked, height));
    paint(
        at(left + width - cap, top, cap, height),
        at(left + width - baked, top, baked, height),
    );
    let stretch = (width - cap * 2.) / (baked - cap * 2.);
    let whole = at(left + cap - cap * stretch, top, baked * stretch, height);
    paint(at(left + cap, top, width - cap * 2., band), whole);
    paint(
        at(left + cap, top + height - band, width - cap * 2., band),
        whole,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(pixels: Vec<[f32; 4]>) -> Icon {
        Icon {
            width: pixels.len(),
            height: 1,
            pixels,
        }
    }

    fn solid(colour: [f32; 3], count: usize) -> Icon {
        icon(vec![[colour[0], colour[1], colour[2], 1.]; count])
    }

    /// An icon of opaque greys that are `spread` either side of a mid grey. Each is √3 × spread
    /// from the mean, as the three channels are.
    fn greys(spread: f32) -> Icon {
        let (dark, light) = (0.5 - spread, 0.5 + spread);
        icon(vec![[dark, dark, dark, 1.], [light, light, light, 1.]])
    }

    #[test]
    fn many_colours_are_colourful() {
        let red_and_blue = icon(vec![[1., 0., 0., 1.], [0., 0., 1., 1.]]);
        assert_eq!(red_and_blue.look(), Some(Look::Colourful));
    }

    #[test]
    fn one_colour_has_its_mean_colour() {
        match solid([0.8, 0.1, 0.2], 9).look() {
            Some(Look::OneColour(mean)) => {
                assert!(
                    mean.iter()
                        .zip([0.8, 0.1, 0.2])
                        .all(|(a, b)| (a - b).abs() < 1e-5)
                )
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_threshold_is_a_mean_distance_of_0_055() {
        // √3 × 0.03 = 0.052, and √3 × 0.035 = 0.061.
        assert!(matches!(greys(0.03).look(), Some(Look::OneColour(_))));
        assert_eq!(greys(0.035).look(), Some(Look::Colourful));
    }

    #[test]
    fn near_white_is_by_luminance_not_by_the_mean() {
        assert_eq!(solid([0.97, 0.97, 0.97], 4).look(), Some(Look::NearWhite));
        // Light, but under 0.88 once its gamma is undone: 0.94 grey is 0.87.
        assert!(matches!(
            solid([0.94, 0.94, 0.94], 4).look(),
            Some(Look::OneColour(_))
        ));
        assert!(matches!(
            solid([1., 0.9, 0.4], 4).look(),
            Some(Look::OneColour(_))
        ));
    }

    #[test]
    fn an_icon_with_no_opaque_pixel_themes_nothing() {
        assert_eq!(icon(vec![[1., 0., 0., 0.]; 6]).look(), None);
        assert_eq!(icon(vec![[1., 0., 0., 0.97]; 6]).look(), None);
    }

    #[test]
    fn pixels_under_0_975_alpha_do_not_count() {
        // The blue ones are too faint to count, so the icon is one colour, however far they are.
        let faint = icon(vec![[1., 0., 0., 1.], [1., 0., 0., 1.], [0., 0., 1., 0.97]]);
        assert!(matches!(faint.look(), Some(Look::OneColour(_))));
        let solid_enough = icon(vec![[1., 0., 0., 1.], [1., 0., 0., 1.], [0., 0., 1., 0.98]]);
        assert_eq!(solid_enough.look(), Some(Look::Colourful));
    }

    #[test]
    fn a_big_icon_is_judged_at_32_px() {
        let big = Icon {
            width: 128,
            height: 64,
            pixels: vec![[0.2, 0.4, 0.6, 1.]; 128 * 64],
        };
        let fitted = big.fitted();
        assert_eq!((fitted.width, fitted.height), (32, 16));
        assert!(
            fitted
                .pixels
                .iter()
                .all(|p| (p[0] - 0.2).abs() < 1e-4 && (p[3] - 1.).abs() < 1e-4)
        );
    }

    #[test]
    fn shrinking_keeps_the_colour_of_pixels_beside_transparent_ones() {
        // Half transparent: the red that's left stays red, at half alpha, not darkened.
        let mut pixels = vec![[1., 0., 0., 1.]; 32 * 64];
        pixels.extend(vec![[0., 0., 0., 0.]; 32 * 64]);
        let halved = Icon {
            width: 64,
            height: 64,
            pixels,
        }
        .fitted();
        let [r, _, _, a] = halved.pixels[0];
        assert!((r - 1.).abs() < 1e-4 && (a - 1.).abs() < 1e-4);
        let [r, _, _, a] = halved.pixels[halved.pixels.len() - 1];
        assert!(r == 0. && a == 0.);
    }

    #[test]
    fn soft_light_with_white_lightens_every_colour() {
        assert_eq!(soft_light_white(0.), 0.);
        assert!((soft_light_white(0.25) - 0.5).abs() < 1e-6);
        assert!((soft_light_white(0.64) - 0.8).abs() < 1e-6);
        assert!((soft_light_white(1.) - 1.).abs() < 1e-6);
    }

    #[test]
    fn blurring_spreads_an_icon_without_adding_to_it() {
        let mut pixels = vec![[0., 0., 0., 0.]; 32 * 32];
        pixels[16 * 32 + 16] = [1., 1., 1., 1.];
        let blurred = Blurred::new(&Icon {
            width: 32,
            height: 32,
            pixels,
        });
        let total: f32 = blurred.pixels.iter().map(|p| p[3]).sum();
        assert!((total - 1.).abs() < 1e-3, "{total}");
        assert!(blurred.pixels[16 * 32 + 20][3] > 0.);
    }
}
