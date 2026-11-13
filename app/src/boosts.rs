//! Boosts (#695, docs/ux-spec.md › Boosts): the CSS the user gives a site, and Zap, which hides
//! what they click there. A profile's Boosts are plain files in its folder, one a site,
//! `Boosts/<site>.css`, so the user can keep them in git, edit them in any editor and see each
//! save at once. The sites they turned off are a list next to the folder, not in it, so a Boost
//! copied from another computer comes on. Settings' switch turns them all off.
//!
//! The app keeps them here and hands the core each site's CSS as it changes (Change), which the
//! core shows on the site's pages as they load and at once on those open. A site is its host
//! without "www.", as the core reads it. Agents get no path to any of it: what a Boost changes is
//! the user's own view of their pages (principle 3, decision 8).
//!
//! A site's script, `Boosts/<site>.js` (#2297), is the user's too: Edit Script… starts and opens
//! it, and it runs only once the user turns it on in the browser.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use gpui::SharedString;

/// The folder in a profile's folder that holds its Boosts.
pub const FOLDER: &str = "Boosts";
/// The sites whose Boost the user turned off, one a line, beside the folder.
pub const OFF_FILE: &str = "Boosts off";
/// What ends a rule Zap wrote, which Undo takes back and the card counts.
const ZAPPED: &str = "/* Zap */";

/// The Boost key of `url`: its host without "www.", for an http(s) page, as the core's
/// Boosts::SiteOf. None for another page, or a host that can't name a file, such as an IPv6
/// address.
pub fn site_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = match host.strip_prefix("www.") {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        _ => host,
    };
    let fits = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-';
    let named = !host.is_empty() && !host.starts_with('.') && host.bytes().all(fits);
    named.then_some(host)
}

/// `css` with a rule that hides what `selector` finds, as a click with Zap asks.
pub fn add_zap(css: &str, selector: &str) -> String {
    let mut css = css.to_owned();
    if !css.is_empty() && !css.ends_with('\n') {
        css.push('\n');
    }
    css.push_str(&format!(
        "{selector} {{ display: none !important; }} {ZAPPED}\n"
    ));
    css
}

/// `css` without the last rule Zap wrote, or None if it has none.
pub fn undo_zap(css: &str) -> Option<String> {
    let lines: Vec<&str> = css.lines().collect();
    let last = lines
        .iter()
        .rposition(|line| line.trim_end().ends_with(ZAPPED))?;
    let kept: Vec<&str> = (lines.iter().enumerate())
        .filter(|(index, _)| *index != last)
        .map(|(_, line)| *line)
        .collect();
    Some(match kept.is_empty() {
        true => String::new(),
        false => kept.join("\n") + "\n",
    })
}

/// What a Boost does, for its card and its row in the Library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// The things Zap hides.
    pub zaps: usize,
    /// The user's own lines of CSS, without blank lines and lines that are only a comment.
    pub lines: usize,
}

impl Summary {
    pub fn of(css: &str) -> Summary {
        let mut summary = Summary::default();
        for line in css.lines().map(str::trim) {
            if line.ends_with(ZAPPED) {
                summary.zaps += 1;
            } else if !line.is_empty() && !(line.starts_with("/*") && line.ends_with("*/")) {
                summary.lines += 1;
            }
        }
        summary
    }

    /// Says it in a sentence: "Hides 3 things and adds 12 lines of your CSS."
    pub fn describe(self) -> String {
        let plural = |count: usize, one: &str, many: &str| match count {
            1 => format!("1 {one}"),
            _ => format!("{count} {many}"),
        };
        let hides = plural(self.zaps, "thing", "things");
        let lines = plural(self.lines, "line", "lines");
        match (self.zaps, self.lines) {
            (0, 0) => "Empty. Zap what you don't want to see, or add your own CSS.".into(),
            (_, 0) => format!("Hides {hides}."),
            (0, _) => format!("Adds {lines} of your CSS."),
            _ => format!("Hides {hides} and adds {lines} of your CSS."),
        }
    }
}

/// A site's CSS as the core should show it now: empty to show none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub profile: u64,
    pub site: String,
    pub css: String,
}

/// A Boost, as the card and the Library show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boost {
    pub site: SharedString,
    pub summary: Summary,
    /// The user hasn't turned it off on its site.
    pub on: bool,
}

/// Each loaded profile's Boosts, and whether Settings has them all off.
#[derive(Default)]
pub struct Boosts {
    profiles: HashMap<u64, Kept>,
    all_off: bool,
    /// Safe start holds them all off for the launch, whatever Settings says (crate::safe_start).
    held: bool,
}

thread_local! {
    static BOOSTS: RefCell<Boosts> = RefCell::new(Boosts::default());
}

/// The UI thread's Boosts. Nothing that calls out, such as to the core, runs inside `change`.
pub fn with<R>(change: impl FnOnce(&mut Boosts) -> R) -> R {
    BOOSTS.with_borrow_mut(change)
}

#[derive(Default)]
struct Kept {
    /// The profile's own folder, which holds FOLDER and OFF_FILE.
    folder: PathBuf,
    sites: BTreeMap<String, String>,
    off: BTreeSet<String>,
    /// What the core was last given, by site, to give it only what changed.
    shown: BTreeMap<String, String>,
}

impl Kept {
    fn boosts(&self) -> PathBuf {
        self.folder.join(FOLDER)
    }

    fn path(&self, site: &str) -> PathBuf {
        self.boosts().join(format!("{site}.css"))
    }

    fn script_path(&self, site: &str) -> PathBuf {
        self.boosts().join(format!("{site}.js"))
    }

    fn read(&mut self) {
        self.sites = read_folder(&self.boosts());
        let off = std::fs::read_to_string(self.folder.join(OFF_FILE)).unwrap_or_default();
        self.off = off.lines().filter_map(site_of_line).collect();
    }

    fn save_off(&self) {
        let path = self.folder.join(OFF_FILE);
        let saved = match self.off.is_empty() {
            true => std::fs::remove_file(&path).or_else(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(error),
            }),
            false => std::fs::write(
                &path,
                self.off
                    .iter()
                    .map(|site| site.clone() + "\n")
                    .collect::<String>(),
            ),
        };
        if let Err(error) = saved {
            crate::log!("couldn't save the Boosts turned off: {error}");
        }
    }

    fn write(&mut self, site: &str, css: &str) -> bool {
        let written = std::fs::create_dir_all(self.boosts())
            .and_then(|_| std::fs::write(self.path(site), css));
        match written {
            Ok(()) => {
                self.sites.insert(site.to_owned(), css.to_owned());
                true
            }
            Err(error) => {
                crate::log!("couldn't save the Boost for {site}: {error}");
                false
            }
        }
    }
}

/// A new Boost's start: a line saying what its file is.
fn first_line(site: &str) -> String {
    format!("/* {site}: your CSS for its pages, shown as you save. */\n\n")
}

fn first_script_line(site: &str) -> String {
    format!("// {site}: your script for its pages, which runs once you turn it on in ilo.\n\n")
}

/// A line of OFF_FILE, if it names a site.
fn site_of_line(line: &str) -> Option<String> {
    let line = line.trim();
    site_of(&format!("https://{line}/")).filter(|site| site == line)
}

/// The Boosts in `folder`, by site: each `<site>.css` that names one.
pub fn read_folder(folder: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return BTreeMap::new();
    };
    (entries.flatten())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let site = site_of_line(name.strip_suffix(".css")?)?;
            Some((site, std::fs::read_to_string(entry.path()).ok()?))
        })
        .collect()
}

impl Boosts {
    /// Whether `profile`'s Boosts are read.
    pub fn loaded(&self, profile: u64) -> bool {
        self.profiles.contains_key(&profile)
    }

    /// Reads `profile`'s Boosts from its folder, `folder`, and says what the core should show.
    pub fn load(&mut self, profile: u64, folder: PathBuf) -> Vec<Change> {
        let kept = self.profiles.entry(profile).or_default();
        kept.folder = folder;
        kept.read();
        self.changes(profile)
    }

    /// Reads `profile`'s Boosts again, as its folder changed, and says what changed.
    pub fn reload(&mut self, profile: u64) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        kept.read();
        self.changes(profile)
    }

    /// The folder `profile`'s Boosts are in, once read.
    pub fn folder(&self, profile: u64) -> Option<PathBuf> {
        Some(self.profiles.get(&profile)?.boosts())
    }

    /// Whether none shows: Settings has them all off, or safe start holds them.
    pub fn all_off(&self) -> bool {
        self.all_off || self.held
    }

    /// Safe start's hold: every Boost off for the launch, leaving Settings' switch as it is.
    pub fn set_held(&mut self, held: bool) -> Vec<Change> {
        self.held = held;
        self.every_change()
    }

    /// Settings' switch: every Boost off, or back as each site has it.
    pub fn set_all_off(&mut self, off: bool) -> Vec<Change> {
        self.all_off = off;
        self.every_change()
    }

    fn every_change(&mut self) -> Vec<Change> {
        let profiles: Vec<u64> = self.profiles.keys().copied().collect();
        profiles
            .into_iter()
            .flat_map(|profile| self.changes(profile))
            .collect()
    }

    /// `site`'s Boost in `profile`, if it has one.
    pub fn get(&self, profile: u64, site: &str) -> Option<Boost> {
        let kept = self.profiles.get(&profile)?;
        let css = kept.sites.get(site)?;
        Some(Boost {
            site: site.to_owned().into(),
            summary: Summary::of(css),
            on: !kept.off.contains(site),
        })
    }

    /// `profile`'s Boosts, by site.
    pub fn list(&self, profile: u64) -> Vec<Boost> {
        let Some(kept) = self.profiles.get(&profile) else {
            return Vec::new();
        };
        (kept.sites.keys())
            .filter_map(|site| self.get(profile, site))
            .collect()
    }

    /// Turns `site`'s Boost on or off there.
    pub fn set_on(&mut self, profile: u64, site: &str, on: bool) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let changed = match on {
            true => kept.off.remove(site),
            false => kept.off.insert(site.to_owned()),
        };
        if changed {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// Adds a rule hiding what `selector` finds to `site`'s Boost, starting one if it has none,
    /// and turns it on: what the user just zapped goes.
    pub fn zap(&mut self, profile: u64, site: &str, selector: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let css = add_zap(kept.sites.get(site).map_or("", String::as_str), selector);
        if kept.write(site, &css) && kept.off.remove(site) {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// Takes back the last rule Zap added to `site`'s Boost.
    pub fn undo_zap(&mut self, profile: u64, site: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        if let Some(css) = kept.sites.get(site).and_then(|css| undo_zap(css)) {
            kept.write(site, &css);
        }
        self.changes(profile)
    }

    /// The editor's edits for `site`'s Boost, the default where it has none.
    pub fn style(&self, profile: u64, site: &str) -> crate::boost_style::Style {
        let css = (self.profiles.get(&profile))
            .and_then(|kept| kept.sites.get(site))
            .map_or("", String::as_str);
        crate::boost_style::Style::of(css)
    }

    /// Rewrites the editor's block in `site`'s Boost as `style`, starting the Boost if it has
    /// none, and turns it on, as the user sees each edit. While `kept` is false, as a drag goes,
    /// the page shows it but the file waits for the drag's end.
    pub fn set_style(
        &mut self,
        profile: u64,
        site: &str,
        style: &crate::boost_style::Style,
        kept: bool,
    ) -> Vec<Change> {
        let Some(kept_boosts) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let before = kept_boosts.sites.get(site).cloned();
        let css = style.into_file(&before.unwrap_or_else(|| first_line(site)));
        let written = match kept {
            true => kept_boosts.write(site, &css),
            false => {
                kept_boosts.sites.insert(site.to_owned(), css);
                true
            }
        };
        if written && kept_boosts.off.remove(site) {
            kept_boosts.save_off();
        }
        self.changes(profile)
    }

    /// Deletes `site`'s Boost: its file, and its place in the list of those turned off.
    pub fn delete(&mut self, profile: u64, site: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        if let Err(error) = std::fs::remove_file(kept.path(site))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log!("couldn't delete the Boost for {site}: {error}");
            return Vec::new();
        }
        kept.sites.remove(site);
        // Its script goes with it.
        if let Err(error) = std::fs::remove_file(kept.script_path(site))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log!("couldn't delete the Boost script for {site}: {error}");
        }
        if kept.off.remove(site) {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// `site`'s file, for the user's editor, started with a line saying what it is if the site has
    /// no Boost yet.
    pub fn file(&mut self, profile: u64, site: &str) -> Option<PathBuf> {
        let kept = self.profiles.get_mut(&profile)?;
        if !kept.sites.contains_key(site) {
            if !kept.write(site, &first_line(site)) {
                return None;
            }
        }
        Some(kept.path(site))
    }

    /// `site`'s script, `Boosts/<site>.js` (#2297), for the user's editor, started with a line
    /// saying what it is if there's none. It runs only once the user turns it on in the browser.
    /// A script is part of its site's Boost, so a site without one gets its CSS started too: the
    /// Library lists it, and Delete… takes both.
    pub fn script_file(&mut self, profile: u64, site: &str) -> Option<PathBuf> {
        self.file(profile, site)?;
        let kept = self.profiles.get_mut(&profile)?;
        let path = kept.script_path(site);
        if !path.exists() {
            let written = std::fs::create_dir_all(kept.boosts())
                .and_then(|_| std::fs::write(&path, first_script_line(site)));
            if let Err(error) = written {
                crate::log!("couldn't start the script for {site}: {error}");
                return None;
            }
        }
        Some(path)
    }

    /// What the core should show that it doesn't yet: each site's CSS, or none for a site turned
    /// off, there no more, or with Settings' switch off.
    fn changes(&mut self, profile: u64) -> Vec<Change> {
        let all_off = self.all_off();
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let sites: BTreeSet<String> =
            (kept.sites.keys().chain(kept.shown.keys()).cloned()).collect();
        let mut changes = Vec::new();
        for site in sites {
            let css = match kept.sites.get(&site) {
                Some(css) if !all_off && !kept.off.contains(&site) => css.clone(),
                _ => String::new(),
            };
            if kept.shown.get(&site).map_or("", String::as_str) == css {
                continue;
            }
            match css.is_empty() {
                true => kept.shown.remove(&site),
                false => kept.shown.insert(site.clone(), css.clone()),
            };
            changes.push(Change { profile, site, css });
        }
        changes
    }
}

/// Calls `changed` on a thread of its own each time a file in `folder` is written, added, renamed
/// or deleted, until the app quits, making the folder if it isn't there. The thread waits on the
/// system's notice, so an idle browser isn't woken (principle 1). Returns whether it watches.
pub fn watch(folder: PathBuf, changed: impl Fn() + Send + 'static) -> bool {
    if let Err(error) = std::fs::create_dir_all(&folder) {
        crate::log!("couldn't make the Boosts folder: {error}");
        return false;
    }
    let started = std::thread::Builder::new()
        .name("boosts".into())
        .spawn(move || watch_on_thread(&folder, changed));
    started.is_ok()
}

/// `watch`, calling `changed` on the UI thread rather than the watcher's.
pub fn watch_in(
    folder: PathBuf,
    cx: &mut gpui::App,
    changed: impl Fn(&mut gpui::App) + 'static,
) -> bool {
    use futures::StreamExt;
    let (told, mut changes) = futures::channel::mpsc::unbounded::<()>();
    if !watch(folder, move || _ = told.unbounded_send(())) {
        return false;
    }
    cx.spawn(async move |cx| {
        while changes.next().await.is_some() {
            cx.update(|cx| changed(cx));
        }
    })
    .detach();
    true
}

#[cfg(target_os = "linux")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
        return;
    };
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return;
    }
    let mask = libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_MOVED_FROM | libc::IN_DELETE;
    if unsafe { libc::inotify_add_watch(fd, path.as_ptr(), mask) } < 0 {
        unsafe { libc::close(fd) };
        return;
    }
    let mut events = [0u8; 4096];
    loop {
        let read = unsafe { libc::read(fd, events.as_mut_ptr().cast(), events.len()) };
        if read < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if read <= 0 {
            break;
        }
        changed();
    }
    unsafe { libc::close(fd) };
}

/// kqueue tells of a change to a file it holds open, and of a folder's names, not its files'
/// contents, so each wait holds the folder and every file in it, opened again after each change.
#[cfg(target_os = "macos")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::unix::ffi::OsStrExt;
    let queue = unsafe { libc::kqueue() };
    if queue < 0 {
        return;
    }
    let open = |path: &Path| {
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        (fd >= 0).then_some(fd)
    };
    loop {
        let files = (std::fs::read_dir(folder).into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "css"));
        let fds: Vec<i32> = std::iter::once(folder.to_path_buf())
            .chain(files)
            .filter_map(|path| open(&path))
            .collect();
        if fds.is_empty() {
            break;
        }
        let watches: Vec<libc::kevent> = (fds.iter())
            .map(|&fd| libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_VNODE,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: libc::NOTE_WRITE
                    | libc::NOTE_EXTEND
                    | libc::NOTE_DELETE
                    | libc::NOTE_RENAME
                    | libc::NOTE_ATTRIB,
                data: 0,
                udata: std::ptr::null_mut(),
            })
            .collect();
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let got = unsafe {
            libc::kevent(
                queue,
                watches.as_ptr(),
                watches.len() as i32,
                &mut event,
                1,
                std::ptr::null(),
            )
        };
        // Closing a file takes its watch out of the queue.
        for fd in fds {
            unsafe { libc::close(fd) };
        }
        if got < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            break;
        }
        if got > 0 {
            changed();
        }
    }
    unsafe { libc::close(queue) };
}

#[cfg(target_os = "windows")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FindCloseChangeNotification,
        FindFirstChangeNotificationW, FindNextChangeNotification,
    };
    use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};
    let path: Vec<u16> = folder.as_os_str().encode_wide().chain([0]).collect();
    let filter = FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE;
    let handle = unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, filter) };
    if handle == INVALID_HANDLE_VALUE {
        return;
    }
    while unsafe { WaitForSingleObject(handle, INFINITE) } == WAIT_OBJECT_0 {
        changed();
        if unsafe { FindNextChangeNotification(handle) } == 0 {
            break;
        }
    }
    unsafe { FindCloseChangeNotification(handle) };
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn watch_on_thread(_: &Path, _: impl Fn()) {}

#[cfg(test)]
mod tests;
