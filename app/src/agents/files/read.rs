//! What read_file makes of a file's bytes (#4367): text as it is, a PDF for the engine's reader,
//! and an image as a picture the assistant looks at. A file is judged by its first bytes, never its
//! name, and its bytes come from the path floor's held handle, so what's judged is what's read.

use std::io::Cursor;

use base64::Engine as _;

use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbImage};

use super::words;

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

/// Why an image or a PDF isn't read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unread {
    /// An image over `MOST_PIXELS`.
    Pixels,
    /// A PDF over `MOST_PDF_BYTES`.
    Bytes,
    /// An image its decoder refused: broken, or not what its first bytes said.
    Broken,
    /// A PDF with a password (BC_PDF_LOCKED).
    Locked,
    /// A PDF the engine couldn't open, or didn't within its time, which it doesn't tell apart
    /// (BC_PDF_BROKEN).
    Damaged,
    /// The read was dropped before its answer (BC_PDF_CLOSED).
    Stopped,
    /// Pages asked from `from` (from 1) of a PDF of `pages`.
    Past { pages: usize, from: usize },
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

/// The lines a text file's read gives when it's asked for none, and at most.
pub const LINES: usize = 2000;
/// The most pages one read gives, as the core reads at most these (BC_PDF_MOST_PAGES).
pub const MOST_PAGES: usize = 30;
/// The most page pictures one answer carries, so a scanned PDF's answer stays one a model reads.
pub const MOST_PICTURES: usize = 10;
/// What an answer's text holds at most, as every tool's does (#3657), less room for its last line.
pub const TEXT_ROOM: usize = crate::agents::tools::MAX_ANSWER - 200;

/// A PDF's pages, as the core read them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pdf {
    /// A BC_PDF_ result.
    pub result: i32,
    pub page_count: usize,
    /// Each page's text, and its picture as JPEG for one with almost no text, as a scan.
    pub pages: Vec<(String, Option<Vec<u8>>)>,
}

pub const PDF_READ: i32 = 0;
pub const PDF_LOCKED: i32 = 1;
pub const PDF_BROKEN: i32 = 2;

/// A PDF's pages as the assistant gets them: their text, the pictures for the model alone, and
/// which pages they are, `(a, b, n)` from 1, where they aren't the whole of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfRead {
    pub text: String,
    pub pictures: Vec<String>,
    pub cut: Option<(usize, usize, usize)>,
}

/// What the assistant gets of a PDF read from page `first` (from 0): each page headed with its
/// number, its picture after the text for a page that has one, as whole pages as fit an answer,
/// and, for a read that isn't the whole PDF, which pages are in it (PD1a). The pictures go to the
/// model alone, never the step. Or why it wasn't read.
pub fn pdf_answer(pdf: &Pdf, first: usize) -> Result<PdfRead, Unread> {
    match pdf.result {
        PDF_READ => {}
        PDF_LOCKED => return Err(Unread::Locked),
        PDF_BROKEN => return Err(Unread::Damaged),
        _ => return Err(Unread::Stopped),
    }
    let total = pdf.page_count;
    if first >= total {
        return Err(Unread::Past {
            pages: total,
            from: first + 1,
        });
    }
    let (mut text, mut pictures) = (String::new(), Vec::new());
    let mut read = 0;
    for (index, (words, picture)) in pdf.pages.iter().enumerate() {
        let number = first + index + 1;
        let mut page = format!("Page {number} of {total}\n\n");
        if !words.trim().is_empty() {
            page.push_str(&format!("{}\n\n", words.trim()));
        }
        if picture.is_some() {
            page.push_str(&format!(
                "[Its picture is picture {} below.]\n\n",
                pictures.len() + 1
            ));
        }
        let room = text.len() + page.len() > TEXT_ROOM;
        let crowded = picture.is_some() && pictures.len() >= MOST_PICTURES;
        // A page is never cut: one that doesn't fit is the next read's.
        if read > 0 && (room || crowded) {
            break;
        }
        text.push_str(&page);
        if let Some(jpeg) = picture {
            pictures.push(base64::engine::general_purpose::STANDARD.encode(jpeg));
        }
        read += 1;
    }
    let pages = (first + 1, first + read, total);
    let cut = (pages != (1, total, total)).then_some(pages);
    if let Some(pages) = cut {
        text.push_str(&words::pages_included(pages));
    }
    Ok(PdfRead {
        text: text.trim_end().to_string(),
        pictures,
        cut,
    })
}

#[cfg(test)]
mod tests;
