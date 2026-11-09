//! Portrait Mode (#673): Arc's Capture in Portrait Mode, a picture of the page to share. The page,
//! at its window's corners and over a soft shadow, sits on a background the user picks: the
//! desktop's picture, white or one of the theme picker's colours, as Arc's offers its wallpaper,
//! white or a colour from its slider. The sizes are ours (`[capture.portrait]`), as Arc's weren't
//! recorded, and nothing is written on the picture, where Arc puts its mark.

use image::{RgbaImage, imageops};

use crate::tokens::capture::portrait::{MARGIN, SHADOW, SHADOW_BLUR, SHADOW_Y};

/// What the page sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    /// The desktop's picture, as Arc's first. Where the system gives none, white.
    Desktop,
    White,
    /// One of the theme picker's colours, 0xrrggbb.
    Colour(u32),
}

impl Background {
    /// The name it's kept and logged as: "desktop", "white" or "#rrggbb".
    pub fn name(self) -> String {
        match self {
            Background::Desktop => "desktop".into(),
            Background::White => "white".into(),
            Background::Colour(colour) => format!("#{colour:06x}"),
        }
    }

    pub fn parse(name: &str) -> Option<Background> {
        match name.trim() {
            "desktop" => Some(Background::Desktop),
            "white" => Some(Background::White),
            name => {
                let hex = name.strip_prefix('#')?;
                (hex.len() == 6)
                    .then(|| u32::from_str_radix(hex, 16).ok())
                    .flatten()
                    .map(Background::Colour)
            }
        }
    }
}

/// What the picture's ground is made of, once the desktop's picture is read.
pub enum Ground {
    Colour([u8; 3]),
    Picture(RgbaImage),
}

impl Ground {
    /// `background`'s ground, with `desktop`, the desktop's picture as a PNG, if there's one.
    pub fn of(background: Background, desktop: Option<&[u8]>) -> Ground {
        let white = Ground::Colour([255; 3]);
        match background {
            Background::White => white,
            Background::Colour(colour) => {
                Ground::Colour([(colour >> 16) as u8, (colour >> 8) as u8, colour as u8])
            }
            Background::Desktop => desktop
                .and_then(|png| image::load_from_memory(png).ok())
                .map_or(white, |picture| Ground::Picture(picture.into_rgba8())),
        }
    }
}

/// The distance from `(x, y)` to the edge of the box `width` by `height` at the origin with corners
/// of `radius`: negative inside.
fn distance(x: f32, y: f32, width: f32, height: f32, radius: f32) -> f32 {
    let (half_w, half_h) = (width / 2., height / 2.);
    let qx = (x - half_w).abs() - half_w + radius;
    let qy = (y - half_h).abs() - half_h + radius;
    let outside = (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt();
    outside + qx.max(qy).min(0.) - radius
}

fn mix(under: &mut [u8], over: [u8; 3], amount: f32) {
    for (under, over) in under.iter_mut().zip(over) {
        let mixed = f32::from(*under) + (f32::from(over) - f32::from(*under)) * amount;
        *under = mixed.round() as u8;
    }
}

/// The picture: `page`, in pixels at `scale` to a point, with corners of `radius` points, over its
/// shadow on `ground`, MARGIN points around it.
pub fn compose(page: &RgbaImage, ground: &Ground, scale: f32, radius: f32) -> RgbaImage {
    let margin = (MARGIN * scale).round() as u32;
    let (page_w, page_h) = page.dimensions();
    let (width, height) = (page_w + 2 * margin, page_h + 2 * margin);
    let mut picture = match ground {
        Ground::Colour([r, g, b]) => RgbaImage::from_pixel(width, height, [*r, *g, *b, 255].into()),
        // As the desktop shows it: filling the picture, its middle kept.
        Ground::Picture(desktop) => {
            let (desk_w, desk_h) = desktop.dimensions();
            let fill = (width as f32 / desk_w as f32).max(height as f32 / desk_h as f32);
            let (fill_w, fill_h) = (
                ((desk_w as f32 * fill).ceil() as u32).max(width),
                ((desk_h as f32 * fill).ceil() as u32).max(height),
            );
            let filled = imageops::resize(desktop, fill_w, fill_h, imageops::FilterType::Triangle);
            let (left, top) = ((fill_w - width) / 2, (fill_h - height) / 2);
            let mut picture = imageops::crop_imm(&filled, left, top, width, height).to_image();
            for pixel in picture.pixels_mut() {
                pixel.0[3] = 255;
            }
            picture
        }
    };
    let (radius, blur, drop) = (radius * scale, SHADOW_BLUR * scale, SHADOW_Y * scale);
    let (page_w, page_h, margin) = (page_w as f32, page_h as f32, margin as f32);
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        // Each pixel at its middle, from the page's top left.
        let (px, py) = (x as f32 + 0.5 - margin, y as f32 + 0.5 - margin);
        let shade = distance(px, py - drop, page_w, page_h, radius);
        // Darkest under the page, fading to none `blur` outside it.
        let fade = (1. - (shade + blur / 2.) / blur).clamp(0., 1.);
        mix(&mut pixel.0[..3], [0; 3], SHADOW * fade * fade);
        let cover = (0.5 - distance(px, py, page_w, page_h, radius)).clamp(0., 1.);
        if cover > 0. {
            let from = page.get_pixel(
                (px.max(0.) as u32).min(page.width() - 1),
                (py.max(0.) as u32).min(page.height() - 1),
            );
            let alpha = f32::from(from.0[3]) / 255.;
            mix(
                &mut pixel.0[..3],
                [from.0[0], from.0[1], from.0[2]],
                cover * alpha,
            );
        }
    }
    picture
}

/// `png`, the page's picture, `width` points wide, as Portrait Mode's PNG with its size in pixels,
/// or none if it doesn't decode.
pub fn portrait_png(
    png: &[u8],
    ground: &Ground,
    width: f32,
    radius: f32,
) -> Option<(Vec<u8>, (u32, u32))> {
    let page = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let page = page.into_rgba8();
    // In pixels at the display's scale, as the core gives it.
    let scale = (page.width() as f32 / width.max(1.)).max(1.);
    let picture = compose(&page, ground, scale, radius);
    let size = picture.dimensions();
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some((out.into_inner(), size))
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    #[test]
    fn a_background_is_kept_by_its_name() {
        for background in [
            Background::Desktop,
            Background::White,
            Background::Colour(0x7eb6d5),
        ] {
            assert_eq!(Background::parse(&background.name()), Some(background));
        }
        assert_eq!(Background::parse("#12345"), None);
        assert_eq!(Background::parse("blue"), None);
    }

    /// The page sits MARGIN in on the ground, its corners round and its shadow under it.
    #[test]
    fn the_page_sits_on_its_ground_with_its_corners_and_shadow() {
        let page = RgbaImage::from_pixel(400, 300, Rgba([10, 200, 30, 255]));
        let ground = Ground::of(Background::Colour(0xe1676f), None);
        let picture = compose(&page, &ground, 2., 10.);
        let margin = (MARGIN * 2.) as u32;
        assert_eq!(picture.dimensions(), (400 + 2 * margin, 300 + 2 * margin));
        assert_eq!(
            picture.get_pixel(0, 0).0,
            [0xe1, 0x67, 0x6f, 255],
            "the ground"
        );
        let middle = picture.get_pixel(margin + 200, margin + 150);
        assert_eq!(middle.0, [10, 200, 30, 255], "the page");
        // A corner of the page is cut round, so the ground shows there.
        let corner = picture.get_pixel(margin, margin);
        assert_ne!(
            corner.0,
            [10, 200, 30, 255],
            "the page's corner isn't round"
        );
        // Under the page is darker than the ground, and above it hardly.
        let under = picture.get_pixel(margin + 200, margin + 300 + 4);
        let over = picture.get_pixel(margin + 200, margin - 4);
        assert!(
            under.0[0] < over.0[0],
            "no shadow under the page: {under:?} {over:?}"
        );
    }

    /// The desktop's picture fills the ground, its middle kept; with none, it's white.
    #[test]
    fn the_desktops_picture_fills_the_ground() {
        let page = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        let mut desktop = RgbaImage::from_pixel(64, 32, Rgba([0, 0, 255, 255]));
        for y in 0..32 {
            desktop.put_pixel(0, y, Rgba([255, 0, 0, 255]));
        }
        let mut png = std::io::Cursor::new(Vec::new());
        desktop.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let ground = Ground::of(Background::Desktop, Some(png.get_ref()));
        let picture = compose(&page, &ground, 1., 0.);
        assert_eq!(
            picture.get_pixel(0, 0).0,
            [0, 0, 255, 255],
            "the sides cut off"
        );
        let white = Ground::of(Background::Desktop, None);
        assert_eq!(compose(&page, &white, 1., 0.).get_pixel(0, 0).0, [255; 4]);
    }
}
