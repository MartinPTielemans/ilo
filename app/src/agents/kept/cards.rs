//! What a kept thread brings back across a quit (#4090), in a folder of its own,
//! `<profile>/Kept threads/<card id>/`, which only its owner can open: its card's line, its
//! page's last picture and its log as drawn closed (log.rs), and later its trace's pictures, each
//! under a name this file
//! gives it. This is the one place that writes, reads and removes them, and its `Queue` the one
//! way they're written and removed, in the order asked. The names in it are this file's own and
//! the card's id, never anything of a thread's, and nothing here logs.
//!
//! Written at the thread's Done, off the UI's thread; read after a launch's first frame, off it
//! too, after the launch's sweep; removed
//! with the thread, with Remove All, with its profile, and at launch where no kept record holds
//! the card's id. Its own folder, not the conversation's under `Kept tasks`, as nothing is ever
//! put in that one (kept.rs).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gpui::{BackgroundExecutor, RenderImage};

/// The folder under the profile's that holds each kept thread's own.
pub const FOLDER: &str = "Kept threads";
/// The most of a line the file gives back, in bytes: a drawn line is far shorter, so only a file
/// someone changed has more.
const LINE_MOST: u64 = 4096;
/// The largest picture read back, in pixels a side: a card's is a few hundred.
const SIDE_MOST: u32 = 2048;

/// A card's id: 128 random bits as hex, the browser's own, on its kept record beside the conversation's
/// id. It names the card's folder and is in nothing else.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CardId(String);

impl CardId {
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).expect("the system has no random numbers");
        CardId(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// `text` as an id, if it has an id's shape: nothing else names a folder, so a file someone
    /// changed can't point one elsewhere.
    pub fn parse(text: &str) -> Option<Self> {
        let shaped = text.len() == 32
            && (text.bytes()).all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        shaped.then(|| CardId(text.to_string()))
    }

    /// The id itself, for the file of ids and its folder's name alone.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CardId {
    fn default() -> Self {
        Self::new()
    }
}

/// None of it, so nothing that prints a value prints an id.
impl std::fmt::Debug for CardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CardId(…)")
    }
}

/// The files a card's folder holds: these and no others.
#[derive(Clone, Copy)]
enum File {
    Line,
    Picture,
    Log,
}

impl File {
    fn name(self) -> &'static str {
        match self {
            File::Line => "line",
            File::Picture => "card.png",
            File::Log => "log",
        }
    }
}

/// A picture as GPUI draws it: `width` by `height` pixels, 4 bytes each in BGRA order.
#[derive(Clone, PartialEq)]
pub struct Pixels {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Its size alone.
impl std::fmt::Debug for Pixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pixels({}x{})", self.width, self.height)
    }
}

/// What a write does with a card's picture.
pub enum Picture {
    /// Leaves the one there, or none.
    Leave,
    /// Takes the one there out.
    Remove,
    /// Puts this one in its place.
    Write(Pixels),
    /// Puts this one, as GPUI drew it, in its place: its pixels are read off the UI's thread.
    Taken(Arc<RenderImage>),
}

/// Which it is, and a picture's size alone.
impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Picture::Leave => f.write_str("Leave"),
            Picture::Remove => f.write_str("Remove"),
            Picture::Write(pixels) => write!(f, "Write({pixels:?})"),
            Picture::Taken(_) => f.write_str("Taken(…)"),
        }
    }
}

/// `image`'s pixels, as GPUI drew it.
fn pixels(image: &RenderImage) -> Option<Pixels> {
    let size = image.size(0);
    Some(Pixels {
        bgra: image.as_bytes(0)?.to_vec(),
        width: u32::try_from(size.width.0).ok()?,
        height: u32::try_from(size.height.0).ok()?,
    })
}

/// What a card's folder held.
#[derive(Debug, Default, PartialEq)]
pub struct Card {
    pub line: Option<String>,
    pub picture: Option<Pixels>,
}

/// A kept card's picture as GPUI draws it, made off the UI's thread: equal only to itself, so a
/// card redraws when it comes and not after.
#[derive(Clone)]
pub struct Plate {
    pub image: Arc<RenderImage>,
    pub size: (u32, u32),
}

impl PartialEq for Plate {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.image, &other.image)
    }
}

/// Its size alone.
impl std::fmt::Debug for Plate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plate({}x{})", self.size.0, self.size.1)
    }
}

impl Plate {
    fn of(pixels: Pixels) -> Option<Self> {
        let size = (pixels.width, pixels.height);
        let image = image::RgbaImage::from_raw(size.0, size.1, pixels.bgra)?;
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(image)]));
        Some(Plate { image, size })
    }
}

/// What a launch reads back of a kept thread's card: its line and its picture, ready to draw.
#[derive(Clone, Default)]
pub struct Back {
    pub line: Option<String>,
    pub plate: Option<Plate>,
}

/// None of the line, so nothing that prints a value prints it.
impl std::fmt::Debug for Back {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line = self.line.as_ref().map(|_| "…");
        write!(f, "Back {{ line: {line:?}, plate: {:?} }}", self.plate)
    }
}

/// `id`'s folder in `root`, if it's one of `root`'s own: directly inside it and no link.
fn own_folder(root: &Path, id: &CardId) -> Option<PathBuf> {
    let id = CardId::parse(id.as_str())?;
    let folder = root.join(id.as_str());
    let real = std::fs::symlink_metadata(&folder).ok()?;
    (real.is_dir() && !root.symlink_metadata().ok()?.is_symlink()).then_some(folder)
}

/// A folder only its owner can open, made if it's missing; None if it's a link or can't be made.
fn private_folder(folder: &Path) -> Option<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    match builder.create(folder) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    let real = std::fs::symlink_metadata(folder).ok()?;
    (real.is_dir() && !real.is_symlink()).then_some(())
}

/// Writes `bytes` as a new file only its owner can read, in place of what was there: a link
/// there is taken out, never followed.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    _ = std::fs::remove_file(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

/// `pixels` as a PNG.
fn png(pixels: &Pixels) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut rgba = pixels.bgra.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(
            &rgba,
            pixels.width,
            pixels.height,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(encoded)
}

/// Writes `id`'s card in `root`: its `line` and its `log` in place of what was there, and its
/// picture as `picture` says. With none of them, nothing is left of it. It encodes the picture, so
/// it's never run on the UI's thread. False if any of it couldn't be written, when what was there
/// may be gone.
pub fn write(
    root: &Path,
    id: &CardId,
    line: Option<&str>,
    picture: &Picture,
    log: Option<&str>,
) -> bool {
    write_files(root, id, line, picture, Some(log))
}

/// What a card's write does with its log: writes it, removes the one there, or leaves that one
/// as it is, as a thread whose earlier log and new turn won't fit together under the cap does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Log {
    Write(String),
    Remove,
    Leave,
}

/// `write`, with the log left as it is where `log` is None.
fn write_files(
    root: &Path,
    id: &CardId,
    line: Option<&str>,
    picture: &Picture,
    log: Option<Option<&str>>,
) -> bool {
    if line.is_none() && log == Some(None) && matches!(picture, Picture::Remove) {
        remove(root, id);
        return true;
    }
    let Some(id) = CardId::parse(id.as_str()) else {
        return false;
    };
    let folder = root.join(id.as_str());
    if private_folder(root)
        .and_then(|_| private_folder(&folder))
        .is_none()
    {
        return false;
    }
    let picture = match picture {
        Picture::Leave => None,
        Picture::Remove => Some(None),
        // One that can't be read or encoded leaves none.
        Picture::Write(pixels) => Some(png(pixels)),
        Picture::Taken(image) => Some(pixels(image).as_ref().and_then(png)),
    };
    let line = Some(line.map(|line| line.as_bytes().to_vec()));
    let log = log.map(|log| log.map(|log| log.as_bytes().to_vec()));
    let mut written = true;
    for (file, bytes) in [
        (File::Line, line),
        (File::Picture, picture),
        (File::Log, log),
    ] {
        let Some(bytes) = bytes else {
            continue;
        };
        let path = folder.join(file.name());
        written &= match bytes {
            Some(bytes) => write_new(&path, &bytes).is_ok(),
            None => {
                _ = std::fs::remove_file(&path);
                !path.exists()
            }
        };
    }
    written
}

/// What `id`'s card in `root` holds: each file that's there, reads whole and has its shape. It
/// decodes the picture, so it's never run on the UI's thread. Its log is read alone, by
/// `read_log`, as a launch reads cards and never logs.
pub fn read(root: &Path, id: &CardId) -> Card {
    let Some(folder) = own_folder(root, id) else {
        return Card::default();
    };
    Card {
        line: read_line(&folder.join(File::Line.name())),
        picture: read_picture(&folder.join(File::Picture.name())),
    }
}

/// `id`'s log in `root`, if its file is no larger than a log may be and is one the app wrote.
pub fn read_log(root: &Path, id: &CardId) -> Option<Vec<super::log::Record>> {
    log_file(&own_folder(root, id)?.join(File::Log.name()))
}

fn log_file(path: &Path) -> Option<Vec<super::log::Record>> {
    use std::io::Read;
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let most = super::log::MOST as u64;
    let mut bytes = Vec::new();
    (std::fs::File::open(path).ok()?.take(most + 1))
        .read_to_end(&mut bytes)
        .ok()?;
    super::log::read(&String::from_utf8(bytes).ok()?)
}

fn read_line(path: &Path) -> Option<String> {
    use std::io::Read;
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    (std::fs::File::open(path).ok()?.take(LINE_MOST + 1))
        .read_to_end(&mut bytes)
        .ok()?;
    let line = String::from_utf8(bytes).ok()?;
    let shaped = line.len() as u64 <= LINE_MOST && !line.chars().any(char::is_control);
    shaped.then_some(line)
}

fn read_picture(path: &Path) -> Option<Pixels> {
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(SIDE_MOST);
    limits.max_image_height = Some(SIDE_MOST);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    let mut bgra = rgba.into_raw();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(Pixels {
        bgra,
        width,
        height,
    })
}

/// Removes `id`'s card from `root`. True once it's gone.
pub fn remove(root: &Path, id: &CardId) -> bool {
    match own_folder(root, id) {
        Some(folder) => std::fs::remove_dir_all(&folder).is_ok() || !folder.exists(),
        None => !root.join(id.as_str()).exists(),
    }
}

/// Removes every card from `root`, and `root` with them.
pub fn remove_all(root: &Path) {
    if std::fs::symlink_metadata(root).is_ok_and(|real| real.is_dir()) {
        _ = std::fs::remove_dir_all(root);
    }
}

/// Removes each card in `root` whose id isn't in `kept`, as a kept record holds each card's id:
/// one whose thread went while the browser couldn't take it out. Then `root`, if that left it
/// empty. It reads the folder, so it's never run on the UI's thread.
pub fn sweep(root: &Path, kept: &[CardId]) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(CardId::parse) else {
            continue;
        };
        if !kept.contains(&id) {
            remove(root, &id);
        }
    }
    // Only if it's empty.
    _ = std::fs::remove_dir(root);
}

/// One piece of the cards' file work, in the folder of cards it names.
#[derive(Debug)]
pub enum Job {
    /// A thread's Done: `write`.
    Write {
        root: PathBuf,
        id: CardId,
        line: Option<String>,
        picture: Picture,
        /// Its log as the file holds it (log.rs), none, or the one there as it is.
        log: Log,
    },
    /// Threads gone for good: `remove`, each.
    Remove(PathBuf, Vec<CardId>),
    /// Remove All: `remove_all`.
    RemoveAll(PathBuf),
    /// A launch: `sweep`.
    Sweep(PathBuf, Vec<CardId>),
    /// A launch, after its sweep: each card `read`, its picture made ready to draw, answered by
    /// card in the order asked.
    Read(
        PathBuf,
        Vec<CardId>,
        futures::channel::oneshot::Sender<Vec<(CardId, Back)>>,
    ),
    /// A thread a run before left, first shown: its card's `read_log`.
    ReadLog(
        PathBuf,
        CardId,
        futures::channel::oneshot::Sender<Option<Vec<super::log::Record>>>,
    ),
}

impl Job {
    fn run(self) {
        match self {
            Job::Write {
                root,
                id,
                line,
                picture,
                log,
            } => {
                let log = match &log {
                    Log::Write(log) => Some(Some(log.as_str())),
                    Log::Remove => Some(None),
                    Log::Leave => None,
                };
                _ = write_files(&root, &id, line.as_deref(), &picture, log)
            }
            Job::Remove(root, ids) => {
                for id in &ids {
                    remove(&root, id);
                }
            }
            Job::RemoveAll(root) => remove_all(&root),
            Job::Sweep(root, kept) => sweep(&root, &kept),
            Job::Read(root, ids, answer) => {
                let back = (ids.into_iter())
                    .map(|id| {
                        let card = read(&root, &id);
                        let plate = card.picture.and_then(Plate::of);
                        (
                            id,
                            Back {
                                line: card.line,
                                plate,
                            },
                        )
                    })
                    .collect();
                // A launch that went on without it, as a window closed meanwhile, wants none.
                _ = answer.send(back);
            }
            Job::ReadLog(root, id, answer) => _ = answer.send(read_log(&root, &id)),
        }
    }
}

/// The one way the cards' file work is done: each job off the UI's thread, one at a time, in the
/// order asked, so a later job always finds what an earlier one left, as a Remove All asked after
/// a Done's write finds the card to take out. A job waits only for the ones before it; with none
/// waiting nothing runs, and nothing is on a timer.
#[derive(Clone, Default)]
pub struct Queue(Arc<Mutex<Waiting>>);

#[derive(Default)]
struct Waiting {
    jobs: VecDeque<Job>,
    /// Whether a run is taking the jobs, which takes one asked while it runs too.
    running: bool,
}

impl Queue {
    /// Puts `job` after those asked before it, starting a run on `executor` if none is under way.
    pub fn ask(&self, job: Job, executor: &BackgroundExecutor) {
        {
            let mut waiting = self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            waiting.jobs.push_back(job);
            if std::mem::replace(&mut waiting.running, true) {
                return;
            }
        }
        let queue = self.clone();
        executor.spawn(async move { queue.run() }).detach();
    }

    /// Takes the jobs in order until there are none.
    fn run(&self) {
        loop {
            let job = {
                let mut waiting = self
                    .0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match waiting.jobs.pop_front() {
                    Some(job) => job,
                    None => {
                        waiting.running = false;
                        return;
                    }
                }
            };
            job.run();
        }
    }
}

#[cfg(test)]
mod tests;
