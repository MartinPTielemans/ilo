//! A downloaded file's preview on Linux, as host_file_preview says: a photo's thumbnail, decoded by
//! the image crate GPUI decodes with, in the formats it has, and a video's picture, from the
//! thumbnailer the system has for its type, as a file manager's is (the freedesktop thumbnail
//! spec). Playing one is left to Chromium's decoder, not GStreamer (#451). Any other file shows
//! its type's icon from the desktop's icon theme, as a file manager's does (the freedesktop icon
//! theme spec), an SVG drawn by GPUI's renderer.

use std::{
    collections::HashMap,
    env,
    ffi::{CStr, c_char},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, LazyLock},
    thread,
    time::{Duration, Instant},
};

use gpui::{DevicePixels, SvgRenderer, SvgSize, size};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage, imageops::FilterType};

/// The longest side of a thumbnail, in pixels, as the Mac's.
const THUMBNAIL: u32 = 96;

/// The side of a file's icon, in pixels, as the Mac's: 26 points on a 2× display.
const ICON: u32 = 52;

/// How long a thumbnailer has, as a damaged file can keep one decoding.
const THUMBNAILER_TIME: Duration = Duration::from_secs(10);

type Done =
    extern "C" fn(id: u64, png: *const u8, size: usize, width: f64, height: f64, image: bool);

#[unsafe(no_mangle)]
pub extern "C" fn host_file_preview(path: *const c_char, id: u64, done: Done) {
    let path = PathBuf::from(unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned());
    let Some(cx) = crate::CX.with_borrow(Clone::clone) else {
        return;
    };
    // A photo's thumbnail reads and decodes its file, which can take a while for a large one, a
    // video's waits on its thumbnailer, and an icon on the portal and the theme's files.
    let preview = cx.background_executor().spawn(async move {
        if let Some(thumbnail) = thumbnail(&path).or_else(|| video_thumbnail(&path, id)) {
            return Some((thumbnail, true));
        }
        if !path.exists() {
            return None;
        }
        let theme = icon_theme().await;
        Some((icon(&path, theme.as_deref())?, false))
    });
    cx.foreground_executor()
        .spawn(async move {
            match preview.await {
                Some(((png, width, height), image)) => {
                    done(id, png.as_ptr(), png.len(), width.into(), height.into(), image)
                }
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
    picture.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).ok()?;
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

/// The icon theme the desktop's settings portal names, as GPUI reads the cursor's there, else
/// GTK's settings file's. None leaves GTK's default.
async fn icon_theme() -> Option<String> {
    if let Some(theme) = portal_setting("org.gnome.desktop.interface", "icon-theme").await {
        return Some(theme);
    }
    let config = env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    ["gtk-4.0", "gtk-3.0"].iter().find_map(|gtk| {
        let settings = ini(&fs::read_to_string(config.join(gtk).join("settings.ini")).ok()?);
        let theme = settings.get("Settings")?.get("gtk-icon-theme-name")?.trim_matches('"');
        Some(theme.to_string()).filter(|theme| !theme.is_empty())
    })
}

/// A string the settings portal has for `namespace`'s `key`.
async fn portal_setting(namespace: &str, key: &str) -> Option<String> {
    use zbus::zvariant::{OwnedValue, Value};
    let connection = zbus::Connection::session().await.ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &(namespace, key),
        )
        .await
        .ok()?;
    let value: OwnedValue = reply.body().deserialize().ok()?;
    // Read wraps the value in a second variant.
    let value = match &*value {
        Value::Value(inner) => inner.as_ref(),
        other => other,
    };
    let setting = value.downcast_ref::<&str>().ok()?;
    Some(setting.to_string()).filter(|setting| !setting.is_empty())
}

/// An icon theme: the directories it's in and its index.theme's sections.
struct Theme {
    dirs: Vec<PathBuf>,
    index: Ini,
}

/// An ini file's sections, such as an index.theme's, each its keys' values.
type Ini = HashMap<String, HashMap<String, String>>;

/// `path`'s type's icon from `theme`, ICON pixels square, as a PNG with its width and height.
fn icon(path: &Path, theme: Option<&str>) -> Option<(Vec<u8>, u32, u32)> {
    // Its own, as the app's is borrowed while the UI asks, and one, as a renderer copies the
    // system's fonts the first time it draws.
    static SVG: LazyLock<SvgRenderer> = LazyLock::new(|| SvgRenderer::new(Arc::new(())));
    let mime = mime_type(path).unwrap_or_else(|| "application/octet-stream".into());
    let themes = themes(theme);
    let file = icon_names(&mime).iter().find_map(|name| {
        themes.iter().find_map(|theme| in_theme(name, theme)).or_else(|| unthemed(name))
    })?;
    let picture = if file.extension().is_some_and(|e| e == "svg") {
        let parsed = SVG.parse_svg(&fs::read(&file).ok()?).ok()?;
        let side = DevicePixels(ICON as i32);
        let drawn = SVG.render_parsed(&parsed, SvgSize::ExactSize(size(side, side))).ok()?;
        let mut pixels = drawn.as_bytes(0)?.to_vec();
        // GPUI's are BGRA.
        pixels.chunks_exact_mut(4).for_each(|pixel| pixel.swap(0, 2));
        DynamicImage::ImageRgba8(RgbaImage::from_raw(ICON, ICON, pixels)?)
    } else {
        let picture = ImageReader::open(&file).ok()?.with_guessed_format().ok()?.decode().ok()?;
        picture.resize_exact(ICON, ICON, FilterType::Lanczos3)
    };
    let mut png = Vec::new();
    picture.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).ok()?;
    Some((png, ICON, ICON))
}

/// The icons for `mime`, the most particular first, as shared-mime-info names them: its own, then
/// its generic one, then its kind's.
fn icon_names(mime: &str) -> Vec<String> {
    let listed = |file: &str| {
        data_dirs().iter().find_map(|dir| {
            let list = fs::read_to_string(dir.join("mime").join(file)).ok()?;
            list.lines().find_map(|line| Some(line.strip_prefix(mime)?.strip_prefix(':')?.into()))
        })
    };
    let kind = mime.split('/').next().unwrap_or("application");
    let mut names = vec![listed("icons").unwrap_or_else(|| mime.replace('/', "-"))];
    names.extend(listed("generic-icons"));
    names.push(format!("{kind}-x-generic"));
    names
}

/// `theme` and the themes it inherits, depth first, then Adwaita and hicolor, which GTK falls back
/// to, each once.
fn themes(theme: Option<&str>) -> Vec<Theme> {
    fn add(name: &str, seen: &mut Vec<String>, themes: &mut Vec<Theme>) {
        if seen.iter().any(|each| each == name) {
            return;
        }
        seen.push(name.to_string());
        let dirs: Vec<_> = icon_dirs().into_iter().map(|dir| dir.join(name)).collect();
        let index = dirs.iter().find_map(|dir| fs::read_to_string(dir.join("index.theme")).ok());
        let Some(index) = index.map(|index| ini(&index)) else {
            return;
        };
        let inherits = index.get("Icon Theme").and_then(|keys| keys.get("Inherits")).cloned();
        themes.push(Theme { dirs: dirs.into_iter().filter(|dir| dir.is_dir()).collect(), index });
        for parent in inherits.unwrap_or_default().split(',').map(str::trim) {
            if !parent.is_empty() {
                add(parent, seen, themes);
            }
        }
    }
    let (mut seen, mut themes) = (Vec::new(), Vec::new());
    for name in theme.into_iter().chain(["Adwaita", "hicolor"]) {
        add(name, &mut seen, &mut themes);
    }
    themes
}

/// The directories icon themes are in, the user's first (the icon theme spec).
fn icon_dirs() -> Vec<PathBuf> {
    let home = env::var_os("HOME").map(|home| PathBuf::from(home).join(".icons"));
    home.into_iter().chain(data_dirs().into_iter().map(|dir| dir.join("icons"))).collect()
}

/// The file of the icon `name` in `theme`: one drawn at ICON pixels or scalable to it, else the
/// smallest larger, which stays sharp scaled down, else the largest.
fn in_theme(name: &str, theme: &Theme) -> Option<PathBuf> {
    let listed = theme.index.get("Icon Theme")?;
    let directories = ["Directories", "ScaledDirectories"]
        .iter()
        .filter_map(|list| listed.get(*list))
        .flat_map(|list| list.split(','))
        .map(str::trim);
    // Nearest first: whether it's smaller, then how many pixels from ICON.
    let mut best: Option<((bool, u32), PathBuf)> = None;
    for directory in directories {
        let Some(keys) = theme.index.get(directory) else {
            continue;
        };
        let number = |key: &str| keys.get(key)?.parse::<u32>().ok();
        let Some(size) = number("Size") else {
            continue;
        };
        let scale = number("Scale").unwrap_or(1);
        let (min, max) = match keys.get("Type").map(String::as_str) {
            Some("Scalable") => {
                (number("MinSize").unwrap_or(size), number("MaxSize").unwrap_or(size))
            }
            Some("Fixed") => (size, size),
            _ => {
                let threshold = number("Threshold").unwrap_or(2);
                (size.saturating_sub(threshold), size + threshold)
            }
        };
        let (min, max) = (min * scale, max * scale);
        let distance = match () {
            _ if min > ICON => (false, min - ICON),
            _ if max < ICON => (true, ICON - max),
            _ => (false, 0),
        };
        if best.as_ref().is_some_and(|(nearest, _)| *nearest <= distance) {
            continue;
        }
        let file = ["png", "svg"].iter().find_map(|extension| {
            let file = format!("{directory}/{name}.{extension}");
            theme.dirs.iter().map(|dir| dir.join(&file)).find(|file| file.is_file())
        });
        if let Some(file) = file {
            best = Some((distance, file));
        }
    }
    best.map(|(_, file)| file)
}

/// The icon `name` outside any theme, in the icon directories or /usr/share/pixmaps.
fn unthemed(name: &str) -> Option<PathBuf> {
    let dirs = icon_dirs().into_iter().chain([PathBuf::from("/usr/share/pixmaps")]);
    let dirs: Vec<_> = dirs.collect();
    ["png", "svg"].iter().find_map(|extension| {
        dirs.iter().map(|dir| dir.join(format!("{name}.{extension}"))).find(|file| file.is_file())
    })
}

/// `text`'s sections, as an index.theme or GTK's settings.ini has them.
fn ini(text: &str) -> Ini {
    let mut sections = Ini::new();
    let mut section = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            section = name.to_string();
        } else if let Some((key, value)) = line.split_once('=') {
            let keys = sections.entry(section.clone()).or_default();
            keys.entry(key.trim().to_string()).or_insert_with(|| value.trim().to_string());
        }
    }
    sections
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
    home.into_iter().chain(shared.split(':').map(PathBuf::from)).collect()
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
