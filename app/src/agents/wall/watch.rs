//! The granted folder's watch (#4101): while a task holds a folder, ilo hears from the system when
//! the folder, or a folder above it, is moved, renamed or deleted, or its volume goes, and the
//! grant ends then as the user's Revoke ends it. It reports once and is gone: a folder put back
//! at the same path isn't granted again.
//!
//! It asks the system only about the folders themselves, never about what's in them, so nothing a
//! task does inside its folder wakes it, and it never polls. It never follows a link, never opens
//! a file and never reads a folder's entries; the Mac's queue holds each folder open for events
//! only.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the folders above the granted one are watched too, so a parent's rename ends the grant.
/// On the Mac, opening Desktop, Documents, Downloads or a removable volume's folder may ask the
/// user for access, which the pick of a folder inside them doesn't cover; the check VM's run
/// decides whether this stays on there. Off, a rename above the folder goes unseen.
const WATCH_ABOVE: bool = true;

/// A folder's own identity, so a folder of the same name, or a link put in its place, isn't it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    device: u64,
    inode: u64,
}

impl Identity {
    /// The folder at `path` itself, never through a link at its end: None when nothing is there,
    /// or a link or a file is.
    pub fn of(path: &Path) -> Option<Identity> {
        Identity::of_found(std::fs::symlink_metadata(path).ok()?)
    }

    /// The folder a handle holds open, by the handle and not its path again: a folder put at the
    /// path after the open isn't taken for the one opened.
    #[cfg(unix)]
    pub fn of_open(dir: std::os::fd::BorrowedFd) -> Option<Identity> {
        let held = std::fs::File::from(dir.try_clone_to_owned().ok()?);
        Identity::of_found(held.metadata().ok()?)
    }

    fn of_found(found: std::fs::Metadata) -> Option<Identity> {
        if !found.file_type().is_dir() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some(Identity {
                device: found.dev(),
                inode: found.ino(),
            })
        }
        // No system without a wall starts a watch, so there's nothing for an identity to tell.
        #[cfg(not(unix))]
        Some(Identity {
            device: 0,
            inode: 0,
        })
    }
}

/// Why a granted folder's grant ended. Only the dev log names which; the user's line and the
/// activity file say only that it's no longer where it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gone {
    /// Moved or renamed, it or a folder above it, or another put in its place.
    Moved,
    Deleted,
    /// Its volume was unmounted or ejected.
    Unmounted,
    /// The system wouldn't watch it, so ilo couldn't tell when it went.
    Unwatched,
}

impl Gone {
    fn kind(self) -> &'static str {
        match self {
            Gone::Moved => "moved",
            Gone::Deleted => "deleted",
            Gone::Unmounted => "unmounted",
            Gone::Unwatched => "unwatched",
        }
    }
}

/// A watch on one task's granted folder, held with the task's wall and dropped with it. Dropping
/// it ends the watch with no report.
pub struct Watch {
    stopped: Arc<AtomicBool>,
    waker: Waker,
    #[cfg(test)]
    thread: Option<std::thread::JoinHandle<()>>,
    #[cfg(test)]
    ready: std::sync::mpsc::Receiver<()>,
}

impl Watch {
    /// Watches `root`, the folder whose identity was `held` at the pick, and calls `gone` once, on
    /// the watch's own thread, when it goes. The opens happen on that thread, so a system prompt
    /// for one never holds up the UI. If the folder isn't `held` by the time the watch is up,
    /// `gone` is called then.
    pub fn start(root: &Path, held: Identity, gone: impl FnOnce(Gone) + Send + 'static) -> Watch {
        let stopped = Arc::new(AtomicBool::new(false));
        #[cfg(test)]
        let (up, ready) = std::sync::mpsc::channel();
        let (waker, queue) = match Waker::new() {
            Ok(made) => made,
            Err(error) => {
                crate::log!("browser: a task's folder watch couldn't start: {error}");
                gone(Gone::Unwatched);
                return Watch {
                    stopped,
                    waker: Waker::none(),
                    #[cfg(test)]
                    thread: None,
                    #[cfg(test)]
                    ready,
                };
            }
        };
        let folders = watched(root);
        let told = stopped.clone();
        // Kept where a thread that never started can't take it, so that's told too.
        let gone = Arc::new(std::sync::Mutex::new(Some(gone)));
        let report = gone.clone();
        let thread = std::thread::Builder::new()
            .name("folder watch".into())
            .spawn(move || {
                let ended = queue.watch(&folders, held, || {
                    #[cfg(test)]
                    let _ = up.send(());
                });
                let gone = report.lock().ok().and_then(|mut gone| gone.take());
                if let Some(why) = ended
                    && let Some(gone) = gone
                    && !told.swap(true, Ordering::SeqCst)
                {
                    crate::log!(
                        "browser: a task's folder went away ({}); ilo ended the grant",
                        why.kind()
                    );
                    gone(why);
                }
            });
        if let Err(error) = &thread {
            crate::log!("browser: a task's folder watch couldn't start: {error}");
            stopped.store(true, Ordering::SeqCst);
            if let Some(gone) = gone.lock().ok().and_then(|mut gone| gone.take()) {
                gone(Gone::Unwatched);
            }
        }
        Watch {
            stopped,
            waker,
            #[cfg(test)]
            thread: thread.ok(),
            #[cfg(test)]
            ready,
        }
    }

    /// Waits for the watch to be up, or ended.
    #[cfg(test)]
    fn up(&self) {
        let _ = self.ready.recv_timeout(std::time::Duration::from_secs(10));
    }

    /// Ends the watch and waits for its thread, which the UI never does: an open may be waiting on
    /// a system prompt.
    #[cfg(test)]
    fn end(mut self) {
        let thread = self.thread.take();
        drop(self);
        if let Some(thread) = thread {
            thread.join().unwrap();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            self.waker.wake();
        }
    }
}

/// The folder, then each folder above it while `WATCH_ABOVE`, up to the root's child: `/` can't
/// be moved.
fn watched(root: &Path) -> Vec<PathBuf> {
    let above = root
        .ancestors()
        .skip(1)
        .filter(|folder| folder.parent().is_some());
    std::iter::once(root)
        .chain(above.take_while(|_| WATCH_ABOVE))
        .map(Path::to_path_buf)
        .collect()
}

/// Whether the watched folders are still the granted folder and the folders on its way: the
/// granted one is `held`, and no folder above it is now a link.
fn still_there(folders: &[PathBuf], held: Identity) -> bool {
    Identity::of(&folders[0]) == Some(held)
        && folders[1..]
            .iter()
            .all(|folder| Identity::of(folder).is_some())
}

#[cfg(target_os = "linux")]
mod system {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    /// Wakes the watch's thread to end it, through an eventfd its poll waits on too.
    pub struct Waker(i32);

    pub struct Queue {
        inotify: i32,
        woken: i32,
    }

    impl Waker {
        pub fn new() -> std::io::Result<(Waker, Queue)> {
            let woken = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
            if woken < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let inotify = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
            if inotify < 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::close(woken) };
                return Err(error);
            }
            // The waker's own copy, so either end closes its own whenever it's done. Without it
            // nothing could end the watch's thread, so the watch doesn't start.
            let shared = unsafe { libc::fcntl(woken, libc::F_DUPFD_CLOEXEC, 0) };
            if shared < 0 {
                let error = std::io::Error::last_os_error();
                unsafe {
                    libc::close(inotify);
                    libc::close(woken);
                }
                return Err(error);
            }
            Ok((Waker(shared), Queue { inotify, woken }))
        }

        pub fn none() -> Waker {
            Waker(-1)
        }

        pub fn wake(&self) {
            if self.0 >= 0 {
                let one: u64 = 1;
                unsafe { libc::write(self.0, (&one as *const u64).cast(), 8) };
            }
        }
    }

    impl Queue {
        /// Watches `folders` until one goes, or the watch is ended, and closes what it opened.
        pub fn watch(self, folders: &[PathBuf], held: Identity, up: impl FnOnce()) -> Option<Gone> {
            let ended = self.wait(folders, held, up);
            unsafe {
                libc::close(self.inotify);
                libc::close(self.woken);
            }
            ended
        }

        fn wait(&self, folders: &[PathBuf], held: Identity, up: impl FnOnce()) -> Option<Gone> {
            // Only the folder's own move, delete and volume: nothing done inside it.
            let mask =
                libc::IN_MOVE_SELF | libc::IN_DELETE_SELF | libc::IN_ONLYDIR | libc::IN_DONT_FOLLOW;
            for folder in folders {
                let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
                    return Some(Gone::Unwatched);
                };
                if unsafe { libc::inotify_add_watch(self.inotify, path.as_ptr(), mask) } < 0 {
                    let error = std::io::Error::last_os_error();
                    return Some(match error.raw_os_error() {
                        Some(libc::ENOENT | libc::ENOTDIR | libc::ELOOP) => Gone::Moved,
                        _ => {
                            crate::log!("browser: a task's folder watch couldn't start: {error}");
                            Gone::Unwatched
                        }
                    });
                }
            }
            // A change between the pick and the watches being up was told of to no one.
            if !still_there(folders, held) {
                return Some(Gone::Moved);
            }
            up();
            let mut events = [0u8; 4096];
            loop {
                let mut waiting = [
                    libc::pollfd {
                        fd: self.inotify,
                        events: libc::POLLIN,
                        revents: 0,
                    },
                    libc::pollfd {
                        fd: self.woken,
                        events: libc::POLLIN,
                        revents: 0,
                    },
                ];
                if unsafe { libc::poll(waiting.as_mut_ptr(), 2, -1) } < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    crate::log!("browser: a task's folder watch stopped: {error}");
                    return Some(Gone::Unwatched);
                }
                if waiting[1].revents != 0 {
                    return None;
                }
                let read = unsafe { libc::read(self.inotify, events.as_mut_ptr().cast(), 4096) };
                if read < 0 {
                    let error = std::io::Error::last_os_error();
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) {
                        continue;
                    }
                    // One that stays would spin the thread, so the watch ends.
                    crate::log!("browser: a task's folder watch stopped: {error}");
                    return Some(Gone::Unwatched);
                }
                // inotify never reads nothing after a poll says it's ready; if it did, it would
                // again, so that ends the watch too.
                if read == 0 {
                    crate::log!("browser: a task's folder watch stopped: it read nothing");
                    return Some(Gone::Unwatched);
                }
                if let Some(gone) = told(&events[..read as usize]) {
                    return Some(gone);
                }
            }
        }
    }

    /// What a read of inotify's events says: the first move, delete or unmount among them.
    fn told(mut events: &[u8]) -> Option<Gone> {
        let head = std::mem::size_of::<libc::inotify_event>();
        while events.len() >= head {
            let event: libc::inotify_event =
                unsafe { std::ptr::read_unaligned(events.as_ptr().cast()) };
            let gone = match event.mask {
                mask if mask & libc::IN_UNMOUNT != 0 => Some(Gone::Unmounted),
                mask if mask & libc::IN_DELETE_SELF != 0 => Some(Gone::Deleted),
                mask if mask & libc::IN_MOVE_SELF != 0 => Some(Gone::Moved),
                // Events were lost, so one of them may have been the folder's.
                mask if mask & libc::IN_Q_OVERFLOW != 0 => Some(Gone::Moved),
                _ => None,
            };
            if gone.is_some() {
                return gone;
            }
            events = &events[(head + event.len as usize).min(events.len())..];
        }
        None
    }

    impl Drop for Waker {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe { libc::close(self.0) };
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod system {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    /// The user event the waker triggers in the queue to end the watch.
    const WAKE: usize = 1;

    pub struct Waker(i32);

    pub struct Queue(i32);

    impl Waker {
        pub fn new() -> std::io::Result<(Waker, Queue)> {
            let queue = unsafe { libc::kqueue() };
            if queue < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let wake = libc::kevent {
                ident: WAKE,
                filter: libc::EVFILT_USER,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            let added =
                unsafe { libc::kevent(queue, &wake, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
            if added < 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::close(queue) };
                return Err(error);
            }
            // The waker's own copy, as on Linux: without it nothing could end the watch's thread.
            let shared = unsafe { libc::fcntl(queue, libc::F_DUPFD_CLOEXEC, 0) };
            if shared < 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::close(queue) };
                return Err(error);
            }
            Ok((Waker(shared), Queue(queue)))
        }

        pub fn none() -> Waker {
            Waker(-1)
        }

        pub fn wake(&self) {
            if self.0 < 0 {
                return;
            }
            let wake = libc::kevent {
                ident: WAKE,
                filter: libc::EVFILT_USER,
                flags: 0,
                fflags: libc::NOTE_TRIGGER,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            unsafe { libc::kevent(self.0, &wake, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        }
    }

    impl Queue {
        pub fn watch(self, folders: &[PathBuf], held: Identity, up: impl FnOnce()) -> Option<Gone> {
            let mut held_open = Vec::new();
            let ended = self.wait(folders, held, up, &mut held_open);
            for fd in held_open.into_iter().chain([self.0]) {
                unsafe { libc::close(fd) };
            }
            ended
        }

        fn wait(
            &self,
            folders: &[PathBuf],
            held: Identity,
            up: impl FnOnce(),
            held_open: &mut Vec<i32>,
        ) -> Option<Gone> {
            for folder in folders {
                let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
                    return Some(Gone::Unwatched);
                };
                let flags =
                    libc::O_EVTONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
                let fd = unsafe { libc::open(path.as_ptr(), flags) };
                if fd < 0 {
                    let error = std::io::Error::last_os_error();
                    return Some(match error.raw_os_error() {
                        Some(libc::ENOENT | libc::ENOTDIR | libc::ELOOP) => Gone::Moved,
                        _ => {
                            crate::log!("browser: a task's folder watch couldn't start: {error}");
                            Gone::Unwatched
                        }
                    });
                }
                held_open.push(fd);
                let watch = libc::kevent {
                    ident: fd as usize,
                    filter: libc::EVFILT_VNODE,
                    flags: libc::EV_ADD | libc::EV_CLEAR,
                    fflags: libc::NOTE_RENAME | libc::NOTE_DELETE | libc::NOTE_REVOKE,
                    data: 0,
                    udata: std::ptr::null_mut(),
                };
                let added = unsafe {
                    libc::kevent(self.0, &watch, 1, std::ptr::null_mut(), 0, std::ptr::null())
                };
                if added < 0 {
                    let error = std::io::Error::last_os_error();
                    crate::log!("browser: a task's folder watch couldn't start: {error}");
                    return Some(Gone::Unwatched);
                }
            }
            // The folder held open is the one granted, or the path names another now.
            if !opened_is(held_open[0], held) || !still_there(folders, held) {
                return Some(Gone::Moved);
            }
            up();
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            loop {
                let got = unsafe {
                    libc::kevent(self.0, std::ptr::null(), 0, &mut event, 1, std::ptr::null())
                };
                if got < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    crate::log!("browser: a task's folder watch stopped: {error}");
                    return Some(Gone::Unwatched);
                }
                if got == 0 {
                    continue;
                }
                if event.filter == libc::EVFILT_USER {
                    return None;
                }
                let gone = match event.fflags {
                    flags if flags & libc::NOTE_REVOKE != 0 => Gone::Unmounted,
                    flags if flags & libc::NOTE_DELETE != 0 => Gone::Deleted,
                    _ => Gone::Moved,
                };
                return Some(gone);
            }
        }
    }

    fn opened_is(fd: i32, held: Identity) -> bool {
        let mut found: libc::stat = unsafe { std::mem::zeroed() };
        let stated = unsafe { libc::fstat(fd, &mut found) };
        stated == 0 && found.st_dev as u64 == held.device && found.st_ino as u64 == held.inode
    }

    impl Drop for Waker {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe { libc::close(self.0) };
            }
        }
    }
}

/// No wall on other systems yet, so no watch: a grant there never comes.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod system {
    use super::*;

    pub struct Waker;
    pub struct Queue;

    impl Waker {
        pub fn new() -> std::io::Result<(Waker, Queue)> {
            Err(std::io::ErrorKind::Unsupported.into())
        }
        pub fn none() -> Waker {
            Waker
        }
        pub fn wake(&self) {}
    }

    impl Queue {
        pub fn watch(self, _: &[PathBuf], _: Identity, _: impl FnOnce()) -> Option<Gone> {
            Some(Gone::Unwatched)
        }
    }
}

use system::Waker;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A temp folder of the test's own, holding `parent/folder`, gone with the test.
    struct Place {
        top: PathBuf,
        parent: PathBuf,
        folder: PathBuf,
    }

    impl Drop for Place {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.top);
        }
    }

    fn place(name: &str) -> Place {
        let top = std::env::temp_dir().join(format!("ilo-watch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&top);
        std::fs::create_dir_all(top.join("parent").join("folder")).unwrap();
        // As the folder panel resolves it: no link on the way, as /tmp is on the Mac.
        let top = top.canonicalize().unwrap();
        let parent = top.join("parent");
        let folder = parent.join("folder");
        Place {
            top,
            parent,
            folder,
        }
    }

    fn watching(folder: &Path) -> (Watch, mpsc::Receiver<Gone>) {
        let (send, told) = mpsc::channel();
        let held = Identity::of(folder).unwrap();
        let watch = Watch::start(folder, held, move |gone| _ = send.send(gone));
        watch.up();
        (watch, told)
    }

    /// The watch's one report, waited for as long as a slow runner may take to tell it.
    fn report(told: &mpsc::Receiver<Gone>) -> Gone {
        told.recv_timeout(Duration::from_secs(10))
            .expect("the watch didn't report the folder gone")
    }

    #[test]
    fn the_folder_moved_into_a_sibling_ends_the_grant() {
        let place = place("sibling");
        let sibling = place.parent.join("sibling");
        std::fs::create_dir(&sibling).unwrap();
        let (_watch, told) = watching(&place.folder);
        std::fs::rename(&place.folder, sibling.join("folder")).unwrap();
        assert_eq!(report(&told), Gone::Moved);
    }

    #[test]
    fn the_folder_renamed_ends_the_grant() {
        let place = place("renamed");
        let (_watch, told) = watching(&place.folder);
        std::fs::rename(&place.folder, place.parent.join("renamed")).unwrap();
        assert_eq!(report(&told), Gone::Moved);
    }

    #[test]
    fn the_folder_deleted_ends_the_grant() {
        let place = place("deleted");
        let (_watch, told) = watching(&place.folder);
        std::fs::remove_dir(&place.folder).unwrap();
        assert_eq!(report(&told), Gone::Deleted);
    }

    #[test]
    fn a_link_of_the_same_name_in_its_place_ends_the_grant() {
        let place = place("link");
        let (_watch, told) = watching(&place.folder);
        let away = place.parent.join("away");
        std::fs::rename(&place.folder, &away).unwrap();
        std::os::unix::fs::symlink(&away, &place.folder).unwrap();
        assert_eq!(report(&told), Gone::Moved);
        let after = told.recv_timeout(Duration::from_secs(10));
        assert_eq!(
            after,
            Err(mpsc::RecvTimeoutError::Disconnected),
            "it reports once"
        );
    }

    #[test]
    fn another_folder_of_the_same_name_in_its_place_ends_the_grant() {
        let place = place("another");
        let (_watch, told) = watching(&place.folder);
        std::fs::rename(&place.folder, place.parent.join("away")).unwrap();
        std::fs::create_dir(&place.folder).unwrap();
        assert_eq!(report(&told), Gone::Moved);
    }

    #[test]
    fn a_parent_renamed_ends_the_grant() {
        let place = place("parent");
        let (_watch, told) = watching(&place.folder);
        std::fs::rename(&place.parent, place.top.join("renamed")).unwrap();
        assert_eq!(report(&told), Gone::Moved);
    }

    /// What a task does inside its folder ends nothing: the watch's one report is the folder's own
    /// delete, which comes after all of it, and nothing came before.
    #[test]
    fn a_change_inside_the_folder_ends_nothing() {
        let place = place("inside");
        let (_watch, told) = watching(&place.folder);
        let note = place.folder.join("note.md");
        std::fs::write(&note, "one").unwrap();
        std::fs::write(&note, "two").unwrap();
        std::fs::rename(&note, place.folder.join("plan.md")).unwrap();
        std::fs::create_dir(place.folder.join("drafts")).unwrap();
        std::fs::rename(place.folder.join("drafts"), place.folder.join("kept")).unwrap();
        std::os::unix::fs::symlink("/", place.folder.join("out")).unwrap();
        assert_eq!(told.try_recv(), Err(mpsc::TryRecvError::Empty));
        std::fs::remove_file(place.folder.join("plan.md")).unwrap();
        std::fs::remove_file(place.folder.join("out")).unwrap();
        std::fs::remove_dir(place.folder.join("kept")).unwrap();
        std::fs::remove_dir(&place.folder).unwrap();
        assert_eq!(report(&told), Gone::Deleted);
    }

    /// A folder swapped between the pick and the watch is told of by no system event, so the
    /// watch's start finds it.
    #[test]
    fn a_folder_swapped_before_the_watch_ends_the_grant_at_its_start() {
        let place = place("swapped");
        let held = Identity::of(&place.folder).unwrap();
        std::fs::rename(&place.folder, place.parent.join("away")).unwrap();
        std::fs::create_dir(&place.folder).unwrap();
        let (send, told) = mpsc::channel();
        let _watch = Watch::start(&place.folder, held, move |gone| _ = send.send(gone));
        assert_eq!(report(&told), Gone::Moved);
    }

    #[test]
    fn a_watch_ended_with_its_task_reports_nothing() {
        let place = place("ended");
        let (watch, told) = watching(&place.folder);
        watch.end();
        std::fs::remove_dir(&place.folder).unwrap();
        assert_eq!(told.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }

    #[test]
    fn only_the_folder_and_the_folders_above_it_are_watched() {
        let watched = watched(Path::new("/home/someone/notes"));
        assert_eq!(
            watched,
            [
                Path::new("/home/someone/notes"),
                Path::new("/home/someone"),
                Path::new("/home")
            ]
        );
    }

    /// The watch's descriptors close with it, whether its task ended it or the folder went. They're
    /// counted in a test process of their own, as other tests open theirs meanwhile.
    #[test]
    fn a_watch_leaves_no_descriptor_open() {
        alone("a_watch_leaves_no_descriptor_open", counted);
    }

    /// Runs `test`, this module's test of that name, in a test process of its own.
    fn alone(name: &str, test: fn()) {
        const ALONE: &str = "ILO_WATCH_ALONE";
        if std::env::var_os(ALONE).is_some() {
            return test();
        }
        let name = format!("agents::wall::watch::tests::{name}");
        let out = crate::child::command(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--test-threads=1"])
            .env(ALONE, "1")
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{name} in a process of its own failed:\n{said}"
        );
        assert!(said.contains("1 passed"), "{name} didn't run:\n{said}");
    }

    /// A watch whose waker can't have its own descriptor couldn't be ended, so it doesn't start:
    /// it reports Unwatched at once and leaves nothing open. The system refuses the copy in a
    /// process of its own, with only the queue's descriptors left free.
    #[test]
    fn a_watch_whose_waker_is_refused_reports_unwatched() {
        alone("a_watch_whose_waker_is_refused_reports_unwatched", refused);
    }

    fn refused() {
        // The queue's own: an eventfd and inotify on Linux, a kqueue on the Mac.
        let queue = match cfg!(target_os = "linux") {
            true => 2,
            false => 1,
        };
        let place = place("refused");
        let held = Identity::of(&place.folder).unwrap();
        let listed = match cfg!(target_os = "linux") {
            true => "/proc/self/fd",
            false => "/dev/fd",
        };
        let highest = std::fs::read_dir(listed)
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u64>().ok())
            .max()
            .unwrap();
        let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) },
            0
        );
        limit.rlim_cur = (highest + 8).min(limit.rlim_max);
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
        let take = || std::fs::File::open("/dev/null");
        let mut taken: Vec<_> = std::iter::from_fn(|| take().ok()).collect();
        taken.truncate(taken.len() - queue);
        let (send, told) = mpsc::channel();
        let _watch = Watch::start(&place.folder, held, move |gone| _ = send.send(gone));
        assert_eq!(
            told.try_recv(),
            Ok(Gone::Unwatched),
            "reported at its start"
        );
        let freed: Vec<_> = std::iter::from_fn(|| take().ok()).collect();
        assert_eq!(freed.len(), queue, "the queue's descriptors closed again");
        drop((taken, freed));
    }

    fn open_descriptors() -> usize {
        let listed = match cfg!(target_os = "linux") {
            true => "/proc/self/fd",
            false => "/dev/fd",
        };
        std::fs::read_dir(listed).unwrap().count()
    }

    fn counted() {
        let place = place("descriptors");
        let before = open_descriptors();
        let (watch, _told) = watching(&place.folder);
        assert!(
            open_descriptors() > before,
            "the watch holds descriptors of its own"
        );
        watch.end();
        assert_eq!(open_descriptors(), before, "after its task ended it");
        let (watch, told) = watching(&place.folder);
        std::fs::remove_dir(&place.folder).unwrap();
        assert_eq!(report(&told), Gone::Deleted);
        watch.end();
        assert_eq!(open_descriptors(), before, "after the folder went");
    }
}
