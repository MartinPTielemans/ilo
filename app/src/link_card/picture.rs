//! A link card's picture, decoded from the bytes the core fetched (bc_link_card_picture): PNG, JPEG,
//! GIF's first frame or WebP, in pure Rust, off the main thread, within limits a hostile file can't
//! get past, and scaled down to what the card draws.

use std::io::Cursor;

use image::{ImageFormat, ImageReader, Limits, RgbaImage, imageops::FilterType};

/// The largest side a picture may claim before it's decoded. Share images are 1200 × 630.
const MAX_SIDE: u32 = 8192;
/// The most a decode may allocate.
const MAX_ALLOC: u64 = 64 << 20;
/// The widest the card keeps it, in pixels: its well at twice its width in points, for 2× screens.
pub const KEEP_WIDTH: u32 = 640;

/// The picture in |bytes|, or None for one that isn't a PNG, JPEG, GIF or WebP, or is broken or
/// over the limits.
pub fn decode(bytes: &[u8]) -> Option<RgbaImage> {
    decode_within(bytes, MAX_SIDE, MAX_ALLOC)
}

fn decode_within(bytes: &[u8], max_side: u32, max_alloc: u64) -> Option<RgbaImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    // By its bytes, not the server's word, and only the four the core asks for.
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP)
    ) {
        return None;
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_side);
    limits.max_image_height = Some(max_side);
    limits.max_alloc = Some(max_alloc);
    reader.limits(limits);
    let picture = reader.decode().ok()?;
    if picture.width() == 0 || picture.height() == 0 {
        return None;
    }
    let picture = if picture.width() > KEEP_WIDTH {
        let height = (u64::from(picture.height()) * u64::from(KEEP_WIDTH)
            / u64::from(picture.width()))
        .max(1) as u32;
        picture.resize_exact(KEEP_WIDTH, height, FilterType::Triangle)
    } else {
        picture
    };
    Some(picture.into_rgba8())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba};

    fn encoded(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
        let picture = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            width,
            height,
            Rgba([200, 40, 40, 255]),
        ));
        let picture = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(picture.to_rgb8())
        } else {
            picture
        };
        let mut bytes = Cursor::new(Vec::new());
        picture.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn decodes_the_four_kinds() {
        for format in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Gif,
            ImageFormat::WebP,
        ] {
            let picture = decode(&encoded(40, 21, format)).unwrap_or_else(|| panic!("{format:?}"));
            assert_eq!(picture.dimensions(), (40, 21), "{format:?}");
        }
    }

    #[test]
    fn wide_pictures_are_kept_at_the_cards_width() {
        let picture = decode(&encoded(1200, 630, ImageFormat::Png)).unwrap();
        assert_eq!(picture.dimensions(), (KEEP_WIDTH, 336));
    }

    #[test]
    fn over_the_limits_is_none() {
        let bytes = encoded(64, 64, ImageFormat::Png);
        assert!(decode_within(&bytes, 64, MAX_ALLOC).is_some());
        assert!(decode_within(&bytes, 63, MAX_ALLOC).is_none());
        assert!(decode_within(&bytes, MAX_SIDE, 1024).is_none());
    }

    #[test]
    fn anything_else_is_none() {
        assert!(decode(b"").is_none());
        assert!(decode(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_none());
        assert!(decode(&encoded(4, 4, ImageFormat::Bmp)).is_none());
        let mut cut = encoded(40, 40, ImageFormat::Png);
        cut.truncate(cut.len() / 2);
        assert!(decode(&cut).is_none());
    }
}
