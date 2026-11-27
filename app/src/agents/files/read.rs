//! What read_file makes of a file's bytes (#4367): text as it is, a PDF for the engine's reader,
//! and an image as a picture the assistant looks at. A file is judged by its first bytes, never its
//! name, and its bytes come from the path floor's held handle, so what's judged is what's read.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbImage};

/// The most bytes of a PDF read_file reads, as they're copied whole to the engine's reader.
pub const MOST_PDF_BYTES: u64 = 200 * 1000 * 1000;
/// The most pixels an image may have to be looked at, checked from its header before any pixel
/// is decoded, so a small file that claims a vast picture costs nothing.
pub const MOST_PIXELS: u64 = 64 * 1000 * 1000;
/// A picture's long side, as large as a model reads a picture whole.
pub const LONGEST_SIDE: u32 = 1568;
const JPEG_QUALITY: u8 = 80;

/// What a file is, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Pdf,
    Image(ImageFormat),
    /// HEIC and other HEIF pictures, which nothing memory-safe decodes: refused as a kind ilo
    /// can't open until the walled worker can decode them apart from the app.
    Heif,
    /// Anything else, refused as a kind ilo can't open.
    Other,
}

/// The kind of a file whose first bytes are `head`, or whose bytes are all of `head` for a short
/// one. Text is valid UTF-8 throughout, so it's judged on the whole file.
pub fn kind_of(head: &[u8]) -> Kind {
    if head.starts_with(b"%PDF-") {
        return Kind::Pdf;
    }
    let image = [
        (&b"\x89PNG\r\n\x1a\n"[..], ImageFormat::Png),
        (&b"\xff\xd8\xff"[..], ImageFormat::Jpeg),
        (&b"GIF87a"[..], ImageFormat::Gif),
        (&b"GIF89a"[..], ImageFormat::Gif),
    ];
    if let Some((_, format)) = image.iter().find(|(magic, _)| head.starts_with(magic)) {
        return Kind::Image(*format);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Kind::Image(ImageFormat::WebP);
    }
    // An ISO media file's first box is its type, whose brand names a HEIF picture.
    const HEIF: [&[u8; 4]; 8] = [
        b"heic", b"heix", b"hevc", b"hevx", b"heim", b"heis", b"mif1", b"msf1",
    ];
    if head.len() >= 12 && &head[4..8] == b"ftyp" && HEIF.iter().any(|brand| &head[8..12] == *brand)
    {
        return Kind::Heif;
    }
    // Text has no NUL, which an ISO media file's boxes and most binaries open with.
    match std::str::from_utf8(head) {
        Ok(text) if !text.contains('\0') => Kind::Text,
        _ => Kind::Other,
    }
}

/// Why an image or a PDF isn't read. Their words are owed to the wording pass (P-bytes, I-pixels),
/// so read_file says them only once the table has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unread {
    /// An image over `MOST_PIXELS`.
    Pixels,
    /// A PDF over `MOST_PDF_BYTES`.
    Bytes,
    /// An image its decoder refused: broken, or not what its first bytes said.
    Broken,
}

/// A picture for the assistant to look at, as JPEG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Whether a PDF of `size` bytes is read at all.
pub fn pdf_fits(size: u64) -> Result<(), Unread> {
    match size <= MOST_PDF_BYTES {
        true => Ok(()),
        false => Err(Unread::Bytes),
    }
}

/// An image's bytes as a picture: its size read from its header and checked first, then decoded
/// within that, its first frame only for an animation, laid on white, as paper is, scaled to at
/// most `LONGEST_SIDE`, and sent as JPEG. Off the UI thread, as a large one takes a while.
pub fn picture(bytes: &[u8], format: ImageFormat) -> Result<Picture, Unread> {
    let reader = || ImageReader::with_format(Cursor::new(bytes), format);
    let (width, height) = reader().into_dimensions().map_err(|_| Unread::Broken)?;
    if u64::from(width) * u64::from(height) > MOST_PIXELS {
        return Err(Unread::Pixels);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    // The decoded picture as RGBA, and room for a decoder's own buffers.
    limits.max_alloc = Some(MOST_PIXELS * 4 + (64 << 20));
    let mut decoding = reader();
    decoding.limits(limits);
    let decoded = decoding.decode().map_err(|_| Unread::Broken)?;
    let scaled = match width.max(height) > LONGEST_SIDE {
        true => decoded.thumbnail(LONGEST_SIDE, LONGEST_SIDE),
        false => decoded,
    };
    let flat = DynamicImage::ImageRgb8(on_white(&scaled));
    let mut jpeg = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY);
    flat.write_with_encoder(encoder)
        .map_err(|_| Unread::Broken)?;
    Ok(Picture {
        jpeg,
        width: flat.width(),
        height: flat.height(),
    })
}

/// `image` with its see-through parts laid on white, as a JPEG has none.
fn on_white(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |channel: u8| {
            let alpha = u16::from(a);
            ((u16::from(channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        };
        image::Rgb([over(r), over(g), over(b)])
    })
}

#[cfg(test)]
mod tests;
