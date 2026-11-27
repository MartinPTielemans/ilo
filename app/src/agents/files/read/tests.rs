use super::*;

use image::{Rgba, RgbaImage};

fn encoded(image: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut Cursor::new(&mut bytes), format)
        .unwrap();
    bytes
}

fn png(width: u32, height: u32) -> Vec<u8> {
    encoded(
        &RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255])),
        ImageFormat::Png,
    )
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\xff\xd8\xff")
}

/// A file is what its first bytes say, whatever its name: so a PDF named .txt is read as a PDF and
/// a PNG named .pdf as a picture, as the name never reaches the judging.
#[test]
fn a_file_is_judged_by_its_first_bytes() {
    assert_eq!(kind_of(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3"), Kind::Pdf);
    assert_eq!(kind_of(&png(2, 2)), Kind::Image(ImageFormat::Png));
    assert_eq!(
        kind_of(b"\xff\xd8\xff\xe0\0\x10JFIF"),
        Kind::Image(ImageFormat::Jpeg)
    );
    assert_eq!(
        kind_of(b"GIF89a\x01\0\x01\0"),
        Kind::Image(ImageFormat::Gif)
    );
    assert_eq!(
        kind_of(b"RIFF\x24\0\0\0WEBPVP8 "),
        Kind::Image(ImageFormat::WebP)
    );
    assert_eq!(kind_of("Taxable income: 41,250 €\n".as_bytes()), Kind::Text);
    assert_eq!(kind_of(b""), Kind::Text);
    assert_eq!(kind_of(b"\0\x01\x02\xfe\xff"), Kind::Other);
    // A text file that starts like a PDF's name isn't one, and a PDF's head is never text.
    assert_eq!(kind_of(b"PDF-1.7 notes"), Kind::Text);
}

/// HEIC is never decoded in the app, whatever its brand, until the walled worker exists.
#[test]
fn a_heic_picture_is_its_own_kind_and_never_an_image() {
    for brand in [b"heic", b"heix", b"mif1", b"msf1"] {
        let mut head = b"\0\0\0\x18ftyp".to_vec();
        head.extend_from_slice(brand);
        head.extend_from_slice(b"\0\0\0\0mif1heic");
        assert_eq!(
            kind_of(&head),
            Kind::Heif,
            "{}",
            String::from_utf8_lossy(brand)
        );
    }
    // A video's type box isn't a picture.
    assert_eq!(kind_of(b"\0\0\0\x18ftypmp42\0\0\0\0"), Kind::Other);
}

#[test]
fn an_image_is_a_jpeg_picture_at_its_own_size_when_small() {
    let picture = picture(&png(300, 200), ImageFormat::Png).unwrap();
    assert!(is_jpeg(&picture.jpeg));
    assert_eq!((picture.width, picture.height), (300, 200));
}

/// A large one is scaled so its long side is 1568, its sides kept in proportion.
#[test]
fn a_large_image_is_scaled_to_its_long_side() {
    let wide = picture(&png(4000, 1000), ImageFormat::Png).unwrap();
    assert_eq!((wide.width, wide.height), (LONGEST_SIDE, 392));
    let tall = picture(&png(1000, 3136), ImageFormat::Png).unwrap();
    assert_eq!((tall.width, tall.height), (500, LONGEST_SIDE));
}

/// PNG's CRC-32.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

/// A picture that claims more pixels than the cap is refused from its header, before a pixel is
/// decoded: this PNG's header says 9000 by 9000 and the file has no pixels at all.
#[test]
fn a_pixel_bomb_is_refused_before_its_decoded() {
    let mut bomb = png(1, 1);
    // IHDR's width and height, after the signature, the chunk's length and its type.
    bomb[16..20].copy_from_slice(&9000u32.to_be_bytes());
    bomb[20..24].copy_from_slice(&9000u32.to_be_bytes());
    // The chunk's CRC, of its type and data, so the header is read rather than refused as broken.
    let crc = crc32(&bomb[12..29]);
    bomb[29..33].copy_from_slice(&crc.to_be_bytes());
    assert_eq!(picture(&bomb, ImageFormat::Png), Err(Unread::Pixels));
    assert!(MOST_PIXELS < 9000 * 9000);
}

#[test]
fn a_broken_image_is_broken() {
    assert_eq!(
        picture(b"\x89PNG\r\n\x1a\nnot really", ImageFormat::Png),
        Err(Unread::Broken)
    );
    // A PNG's bytes said to be a JPEG's.
    assert_eq!(picture(&png(2, 2), ImageFormat::Jpeg), Err(Unread::Broken));
}

/// An animation is looked at by its first frame.
#[test]
fn an_animated_gifs_first_frame_is_the_picture() {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame};
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        for shade in [10u8, 240] {
            let frame = RgbaImage::from_pixel(8, 8, Rgba([shade, shade, shade, 255]));
            encoder
                .encode_frame(Frame::from_parts(
                    frame,
                    0,
                    0,
                    Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
        }
    }
    let picture = picture(&bytes, ImageFormat::Gif).unwrap();
    let first = image::load_from_memory_with_format(&picture.jpeg, ImageFormat::Jpeg).unwrap();
    assert!(
        first.to_rgb8().get_pixel(4, 4).0[0] < 60,
        "the dark first frame"
    );
}

/// What's see-through is white, as on paper, never black.
#[test]
fn a_see_through_image_is_on_white() {
    let clear = encoded(
        &RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0])),
        ImageFormat::Png,
    );
    let picture = picture(&clear, ImageFormat::Png).unwrap();
    let seen = image::load_from_memory_with_format(&picture.jpeg, ImageFormat::Jpeg).unwrap();
    assert!(
        seen.to_rgb8()
            .get_pixel(8, 8)
            .0
            .iter()
            .all(|&channel| channel > 240)
    );
}

#[test]
fn a_pdf_over_its_cap_isnt_read() {
    assert_eq!(pdf_fits(MOST_PDF_BYTES), Ok(()));
    assert_eq!(pdf_fits(MOST_PDF_BYTES + 1), Err(Unread::Bytes));
}
