use serde_json::json;

use super::*;
use crate::agents::files::resolve::NeverIds;
use crate::agents::files::{Never, Row};
use crate::agents::tier::Tier;

/// A made-up home with a granted folder holding a text file, a picture, a PDF, a HEIC and a
/// file of no kind read_file reads, and a key on the never list, in a tree of the test's own. On
/// the Mac it's under the user's caches, as the temporary folder is under /private, which the
/// never list holds.
struct Home {
    base: PathBuf,
    never: Never,
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let picture = image::RgbImage::from_pixel(width, height, image::Rgb([200, 30, 30]));
    (picture.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    ))
    .unwrap();
    bytes
}

impl Home {
    fn new(name: &str) -> Home {
        let root = match cfg!(target_os = "macos") {
            true => PathBuf::from(std::env::var_os("HOME").unwrap()).join("Library/Caches"),
            false => std::env::temp_dir(),
        };
        let base = root.join(format!("ilo-read-file-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let base = std::fs::canonicalize(&base).unwrap();
        let notes: String = (1..=3000).map(|line| format!("line {line}\n")).collect();
        let files: [(&str, Vec<u8>); 8] = [
            ("home/Taxes/notes.txt", notes.into_bytes()),
            (
                "home/Taxes/statement.pdf",
                b"%PDF-1.7 the statement".to_vec(),
            ),
            ("home/Taxes/scan.png", png(40, 20)),
            (
                "home/Taxes/photo.heic",
                b"\0\0\0\x18ftypheic\0\0\0\0".to_vec(),
            ),
            ("home/Taxes/archive.zip", b"PK\x03\x04\xff\xfe\0\0".to_vec()),
            // A PDF by its bytes, whatever its name says.
            ("home/Taxes/looks like text.txt", b"%PDF-1.4 a pdf".to_vec()),
            ("home/Elsewhere/receipt.txt", b"a receipt".to_vec()),
            ("home/.ssh/id_ed25519", b"a key".to_vec()),
        ];
        for (path, bytes) in files {
            let path = base.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let never = Never::of(base.join("home"), vec![base.join("home/profile")], None);
        Home { base, never }
    }

    fn at(&self, path: &str) -> PathBuf {
        self.base.join(path)
    }

    fn folders(&self) -> [Listed; 1] {
        [Listed::for_test(self.at("home/Taxes"), Row::Change)]
    }

    fn read(&self, tier: Tier, arguments: Value) -> Read {
        let folders = self.folders();
        let never_ids = NeverIds::of(&self.never.by_identity());
        let reach = Reach {
            tier,
            folders: &folders,
            attached: None,
            never: &self.never,
            never_ids: &never_ids,
        };
        super::read(&asked(&arguments, &folders).unwrap(), &reach)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn a_path_is_taken_from_the_first_folder_and_pages_are_at_most_thirty() {
    let folders = [Listed::for_test(
        PathBuf::from("/made/up/Taxes"),
        Row::Change,
    )];
    let read = |arguments: Value| asked(&arguments, &folders);
    let notes = read(json!({ "path": "notes.txt" })).unwrap();
    assert_eq!(notes.path, PathBuf::from("/made/up/Taxes/notes.txt"));
    assert_eq!(
        (notes.offset, notes.limit, notes.first, notes.count),
        (1, LINES, 0, MOST_PAGES)
    );
    let paged = |pages: Value| {
        read(json!({ "path": "/a.pdf", "pages": pages })).map(|asked| (asked.first, asked.count))
    };
    assert_eq!(paged(json!("31-60")), Ok((30, 30)));
    assert_eq!(paged(json!("31\u{2013}40")), Ok((30, 10)));
    assert_eq!(paged(json!(5)), Ok((4, 1)));
    assert_eq!(
        paged(json!("1-100")),
        Ok((0, MOST_PAGES)),
        "past 30 is cut to 30"
    );
    for wrong in [json!("0"), json!("9-3"), json!("all"), json!([1])] {
        assert!(paged(wrong.clone()).is_err(), "{wrong}");
    }
    assert!(
        read(json!({ "path": "/a.txt", "limit": 9000 })).is_ok_and(|asked| asked.limit == LINES)
    );
    assert!(read(json!({ "path": "/a.txt", "offset": 0 })).is_err());
    assert!(read(json!({})).is_err());
    // With no folder, a relative path has nothing to be taken from.
    assert!(asked(&json!({ "path": "notes.txt" }), &[]).is_err());
}

/// A text file's lines come numbered, from the offset, up to the limit and an answer's room,
/// with where to read on.
#[test]
fn a_text_file_reads_as_numbered_lines_with_where_to_read_on() {
    let home = Home::new("lines");
    let read = home.read(
        Tier::Look,
        json!({ "path": "notes.txt", "offset": 10, "limit": 3 }),
    );
    assert_eq!(read.name, "notes.txt");
    let Ok(Got::Text(text)) = read.got else {
        panic!("{:?}", read.got)
    };
    assert_eq!(
        text,
        "10\tline 10\n11\tline 11\n12\tline 12\n[Lines 10\u{2013}12 of 3000. Read on with offset 13.]"
    );
    let Ok(Got::Text(text)) = home.read(Tier::Work, json!({ "path": "notes.txt" })).got else {
        panic!()
    };
    assert!(
        text.chars().count() <= crate::agents::tools::MAX_ANSWER,
        "{}",
        text.len()
    );
    assert!(text.starts_with("1\tline 1\n") && text.contains("Read on with offset"));
    let Ok(Got::Text(text)) = home
        .read(Tier::Work, json!({ "path": "notes.txt", "offset": 2999 }))
        .got
    else {
        panic!()
    };
    assert_eq!(text, "2999\tline 2999\n3000\tline 3000\n");
}

/// A file is what its bytes are: a PDF named .txt goes to the engine, a picture is one to look
/// at, and HEIC and a kind read_file doesn't read are refused with the table's lines.
#[test]
fn a_file_is_read_as_what_its_bytes_are() {
    let home = Home::new("kinds");
    let got = |path: &str| home.read(Tier::Work, json!({ "path": path })).got;
    assert!(
        matches!(got("statement.pdf"), Ok(Got::Pdf(bytes)) if bytes == b"%PDF-1.7 the statement")
    );
    assert!(matches!(got("looks like text.txt"), Ok(Got::Pdf(_))));
    assert!(matches!(got("scan.png"), Ok(Got::Picture(picture)) if picture.width == 40));
    assert_eq!(got("photo.heic").unwrap_err(), NotRead::Heif);
    assert_eq!(got("archive.zip").unwrap_err(), NotRead::Other);
    assert_eq!(
        line("photo.heic", Err(NotRead::Heif)),
        format!(
            "Couldn't read \u{201c}photo.heic\u{201d}: {PRODUCT} doesn't open this kind of file"
        )
    );
    assert_eq!(
        line("archive.zip", Err(NotRead::Other)),
        "Couldn't read \u{201c}archive.zip\u{201d}: it isn't text, a PDF or a picture"
    );
    assert_eq!(
        told(NotRead::Other),
        "Not done: read_file reads text, PDFs and pictures, and this file is none of them."
    );
    assert_eq!(line("scan.png", Ok(None)), "Read \u{201c}scan.png\u{201d}");
}

/// The table decides each read: never-listed at every level, outside its folders at Work refused
/// as needing its folder until the ask exists, anywhere else at Full, and what's missing outside
/// its folders refused as anything there is, so a refusal never says whether a file exists.
#[test]
fn a_read_is_judged_by_where_it_is_before_whether_its_there() {
    let home = Home::new("judged");
    let not = |tier, path: &str| home.read(tier, json!({ "path": home.at(path) })).got.err();
    let refused = |why| Some(NotRead::Refused(why));
    for tier in [Tier::Look, Tier::Work, Tier::Full] {
        assert_eq!(
            not(tier, "home/.ssh/id_ed25519"),
            refused(Refused::Never),
            "{tier:?}"
        );
    }
    assert_eq!(
        not(Tier::Work, "home/Elsewhere/receipt.txt"),
        refused(Refused::Guarded)
    );
    assert_eq!(
        not(Tier::Work, "home/Elsewhere/gone.txt"),
        refused(Refused::Guarded)
    );
    assert_eq!(not(Tier::Full, "home/Elsewhere/receipt.txt"), None);
    assert_eq!(
        not(Tier::Full, "home/Elsewhere/gone.txt"),
        refused(Refused::Gone)
    );
    assert_eq!(
        not(Tier::Look, "home/Taxes/gone.txt"),
        refused(Refused::Gone)
    );
    let shown = home.read(
        Tier::Work,
        json!({ "path": home.at("home/Elsewhere/receipt.txt") }),
    );
    assert_eq!(
        line(&shown.name, Err(shown.got.unwrap_err())),
        "Couldn't read \u{201c}receipt.txt\u{201d}: add its folder first"
    );
}

fn pdf(result: i32, page_count: usize, pages: &[(&str, bool)]) -> Pdf {
    Pdf {
        result,
        page_count,
        pages: (pages.iter())
            .map(|(text, picture)| (text.to_string(), picture.then(|| vec![0xff, 0xd8, 0xff])))
            .collect(),
    }
}

/// A PDF's pages are headed with their numbers, a scan's picture follows its page's text by its
/// place, and a read that isn't the whole PDF says which pages it has (PD1a), as its step does
/// (PD1).
#[test]
fn a_pdfs_pages_are_numbered_with_their_pictures_and_which_they_are() {
    let read = pdf(
        PDF_READ,
        40,
        &[("Annual statement", false), ("", true), ("Totals", false)],
    );
    let read = pdf_answer(&read, 30).unwrap();
    assert_eq!(
        read.text,
        "Page 31 of 40\n\nAnnual statement\n\nPage 32 of 40\n\n[Its picture is picture 1 below.]\n\nPage 33 of 40\n\nTotals\n\nOnly pages 31\u{2013}33 of 40 are included. Use the pages parameter to read more."
    );
    assert_eq!(
        (read.pictures.as_slice(), read.cut),
        (&["/9j/".to_string()][..], Some((31, 33, 40)))
    );
    assert_eq!(
        line("statement.pdf", Ok(read.cut)),
        "Read \u{201c}statement.pdf\u{201d} \u{b7} pages 31\u{2013}33 of 40"
    );
    let whole = pdf_answer(&pdf(PDF_READ, 2, &[("One", false), ("Two", false)]), 0).unwrap();
    assert!(
        whole.cut.is_none() && !whole.text.contains("Only pages"),
        "{whole:?}"
    );
    assert_eq!(
        line("statement.pdf", Ok(whole.cut)),
        "Read \u{201c}statement.pdf\u{201d}"
    );
}

/// A page is never cut: one that doesn't fit an answer's room, or past its pictures, is the
/// next read's.
#[test]
fn a_page_that_doesnt_fit_is_the_next_reads() {
    let long = "word ".repeat(1500);
    let read = pdf(
        PDF_READ,
        30,
        &[(&long, false), (&long, false), (&long, false)],
    );
    let read = pdf_answer(&read, 0).unwrap();
    assert_eq!(read.cut, Some((1, 1, 30)));
    // One page read says so in its own form, in the step and to the assistant.
    assert!(
        read.text
            .ends_with("Only page 1 of 30 is included. Use the pages parameter to read more."),
        "{}",
        read.text
    );
    assert_eq!(
        line("statement.pdf", Ok(read.cut)),
        "Read \u{201c}statement.pdf\u{201d} \u{b7} page 1 of 30"
    );
    let scans = vec![("", true); 12];
    let read = pdf_answer(&pdf(PDF_READ, 12, &scans), 0).unwrap();
    assert_eq!(
        (read.pictures.len(), read.cut),
        (MOST_PICTURES, Some((1, 10, 12)))
    );
}

/// A PDF or picture not read has its row's reason in the step and its told text for the
/// assistant: a locked PDF's asks for a copy without its password, never the password.
#[test]
fn a_pdf_or_picture_not_read_says_why() {
    let not = |result| pdf_answer(&pdf(result, 0, &[]), 0).unwrap_err();
    assert_eq!(
        (not(PDF_LOCKED), not(PDF_BROKEN), not(PDF_TIMEOUT), not(3)),
        (
            Unread::Locked,
            Unread::Damaged,
            Unread::TimedOut,
            Unread::Stopped
        )
    );
    let lines = [
        (Unread::Locked, "it's locked with a password".to_string()),
        (Unread::Damaged, "it couldn't be opened".to_string()),
        (Unread::TimedOut, "it took too long to open".to_string()),
        (Unread::Bytes, "it's over 200 MB".to_string()),
        (Unread::Pixels, "it's over 64 megapixels".to_string()),
        (
            Unread::Broken,
            format!("it isn't a picture {PRODUCT} can open"),
        ),
    ];
    for (why, reason) in lines {
        let shown = line("scan.pdf", Err(NotRead::Unread(why)));
        assert_eq!(
            shown,
            format!("Couldn't read \u{201c}scan.pdf\u{201d}: {reason}")
        );
    }
    let locked = told(NotRead::Unread(Unread::Locked));
    assert!(
        locked.contains("a copy without one") && !locked.contains("the password"),
        "{locked}"
    );
    assert_eq!(
        told(NotRead::Unread(Unread::Bytes)),
        format!("Not done: {PRODUCT} reads PDFs up to 200 MB.")
    );
    // One that took too long is told so, and how to ask for less, not that it won't open.
    assert_eq!(
        told(NotRead::Unread(Unread::TimedOut)),
        "Not done: this PDF took too long to open. Try fewer pages with the pages parameter, or go on without it."
    );
    assert_eq!(
        line("scan.pdf", Err(NotRead::Unread(Unread::Stopped))),
        "Couldn't read \u{201c}scan.pdf\u{201d}"
    );
    let past = pdf_answer(&pdf(PDF_READ, 4, &[]), 9).unwrap_err();
    assert_eq!(
        told(NotRead::Unread(past)),
        "Not done: this PDF has 4 pages, so none from page 10."
    );
}

/// What the assistant is told never names a level, as only the user changes it.
#[test]
fn what_the_assistant_is_told_names_no_level() {
    let nots = [
        NotRead::Refused(Refused::Needs(Tier::Full)),
        NotRead::Refused(Refused::Guarded),
        NotRead::Heif,
        NotRead::Other,
        NotRead::TooLarge,
        NotRead::Unread(Unread::Pixels),
        NotRead::Unread(Unread::Bytes),
        NotRead::Unread(Unread::Broken),
        NotRead::Unread(Unread::Locked),
        NotRead::Unread(Unread::Damaged),
        NotRead::Unread(Unread::TimedOut),
        NotRead::Unread(Unread::Stopped),
        NotRead::Unread(Unread::Past { pages: 4, from: 9 }),
        NotRead::Failed,
    ];
    for not in nots {
        let told = told(not);
        assert!(told.starts_with("Not done:"), "{told}");
        for tier in [Tier::Look, Tier::Work, Tier::Full] {
            assert!(!told.contains(tier.label()), "{told}");
        }
    }
}
