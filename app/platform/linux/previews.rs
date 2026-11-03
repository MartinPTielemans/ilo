//! A downloaded file's preview on Linux, as host_file_preview says: a photo's thumbnail, decoded by
//! the image crate GPUI decodes with, in the formats it has, and a video's picture, from the
//! thumbnailer the system has for its type, as a file manager's is (the freedesktop thumbnail
//! spec). Playing one is left to Chromium's decoder, not GStreamer (#451).
//!
//! shortcut: no system icon for other files: those answer with no preview, and the UI shows its
//! own icon.

use std::{
    env,
    ffi::{CStr, c_char},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};

/// The longest side of a thumbnail, in pixels, as the Mac's.
const THUMBNAIL: u32 = 96;

/// How long a thumbnailer has, as a damaged file can keep one decoding.
const THUMBNAILER_TIME: Duration = Duration::from_secs(10);

type Done =
    extern "C" fn(id: u64, png: *const u8, size: usize, width: f64, height: f64, image: bool);

#[unsafe(no_mangle)]
pub extern "C" fn host_file_preview(path: *const c_char, id: u64, done: Done) {
    let path = PathBuf::from(
        unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .into_owned(),
    );
    let Some(cx) = crate::CX.with_borrow(Clone::clone) else {
        return;
    };
    // A photo's thumbnail reads and decodes its file, which can take a while for a large one, and
    // a video's waits on its thumbnailer.
    let thumbnail = cx
        .background_executor()
        .spawn(async move { thumbnail(&path).or_else(|| video_thumbnail(&path, id)) });
    cx.foreground_executor()
        .spawn(async move {
            match thumbnail.await {
                Some((png, width, height)) => done(
                    id,
                    png.as_ptr(),
                    png.len(),
                    width.into(),
                    height.into(),
                    true,
                ),
                None => done(id, std::ptr::null(), 0, 0., 0., false),
            }
        })
        .detach();
}

/// The picture at `path` as a PNG at most THUMBNAIL pixels on its longer side, upright as its
/// camera meant, with its width and height; none if it isn't a picture the image crate reads.
fn thumbnail(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok()?;
    let mut picture = DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    if picture.width().max(picture.height()) > THUMBNAIL {
        picture = picture.thumbnail(THUMBNAIL, THUMBNAIL);
    }
    let mut png = Vec::new();
    picture
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some((png, picture.width(), picture.height()))
}

/// The video at `path`'s picture as `thumbnail` gives a photo's, drawn by the thumbnailer the
/// system has for its type; none for another type, or with no thumbnailer for it.
fn video_thumbnail(path: &Path, id: u64) -> Option<(Vec<u8>, u32, u32)> {
    let mime = mime_type(path)?;
    if !mime.starts_with("video/") {
        return None;
    }
    let command = thumbnailer(&mime)?;
    let out = env::temp_dir().join(format!("ilo-thumbnail-{}-{id}.png", std::process::id()));
    let ran = run(&command, path, &out);
    let picture = ran.then(|| thumbnail(&out)).flatten();
    let _ = fs::remove_file(&out);
    picture
}

/// The directories shared-mime-info and thumbnailers are in, the user's first (the XDG base
/// directory spec).
fn data_dirs() -> Vec<PathBuf> {
    let home = env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let shared = env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(shared.split(':').map(PathBuf::from))
        .collect()
}

/// `path`'s MIME type by its name, as shared-mime-info's globs give it, the heaviest first. Only a
/// suffix's globs, as a video's are.
fn mime_type(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_lowercase();
    let mut best: Option<(u32, usize, String)> = None;
    for dir in data_dirs() {
        let Ok(globs) = fs::read_to_string(dir.join("mime/globs2")) else {
            continue;
        };
        for line in globs.lines().filter(|line| !line.starts_with('#')) {
            let mut fields = line.split(':');
            let (Some(weight), Some(mime), Some(glob)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let suffix = glob.strip_prefix('*');
            let Some(suffix) = suffix.filter(|suffix| !suffix.contains(['*', '?', '['])) else {
                continue;
            };
            let matches = match fields.any(|flag| flag.split(',').any(|flag| flag == "cs")) {
                true => name.ends_with(suffix),
                false => lower.ends_with(&suffix.to_lowercase()),
            };
            let weight = weight.parse().unwrap_or(50);
            // The heaviest, then the longest.
            let heavier = |(w, len, _): &(u32, usize, String)| (weight, suffix.len()) > (*w, *len);
            if matches && best.as_ref().is_none_or(heavier) {
                best = Some((weight, suffix.len(), mime.to_string()));
            }
        }
    }
    best.map(|(.., mime)| mime)
}

/// The Exec line of the first thumbnailer for `mime` whose program is installed.
fn thumbnailer(mime: &str) -> Option<String> {
    for dir in data_dirs() {
        let Ok(entries) = fs::read_dir(dir.join("thumbnailers")) else {
            continue;
        };
        let mut files: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        files.retain(|file| file.extension().is_some_and(|e| e == "thumbnailer"));
        files.sort();
        for file in &files {
            let Ok(entry) = fs::read_to_string(file) else {
                continue;
            };
            let key = |name: &str| {
                entry.lines().find_map(|line| {
                    let (key, value) = line.split_once('=')?;
                    (key.trim() == name).then(|| value.trim().to_string())
                })
            };
            let types = key("MimeType").unwrap_or_default();
            if !types.split(';').any(|each| each == mime) {
                continue;
            }
            if key("TryExec").is_some_and(|program| !installed(&program)) {
                continue;
            }
            if let Some(exec) = key("Exec") {
                return Some(exec);
            }
        }
    }
    None
}

/// Whether `program`, a path or a name on PATH, is there to run.
fn installed(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    let dirs = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&dirs).any(|dir| dir.join(program).is_file())
}

/// Runs a thumbnailer's Exec line for `input`, writing `out`, and returns whether it did in time.
fn run(exec: &str, input: &Path, out: &Path) -> bool {
    let mut args = exec.split_whitespace().map(|arg| expand(arg, input, out));
    let Some(program) = args.next() else {
        return false;
    };
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success() && out.is_file(),
            Ok(None) if start.elapsed() < THUMBNAILER_TIME => {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// An Exec argument with the spec's fields filled in: %i the file, %u its URI, %o the picture's
/// file and %s its size.
fn expand(arg: &str, input: &Path, out: &Path) -> String {
    let mut expanded = String::new();
    let mut chars = arg.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            expanded.push(c);
            continue;
        }
        match chars.next() {
            Some('i') => expanded.push_str(&input.to_string_lossy()),
            Some('u') => expanded.push_str(&uri(input)),
            Some('o') => expanded.push_str(&out.to_string_lossy()),
            Some('s') => expanded.push_str(&THUMBNAIL.to_string()),
            Some(other) => expanded.push(other),
            None => {}
        }
    }
    expanded
}

/// `path`'s file URI, its bytes past the unreserved ones escaped.
fn uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}
