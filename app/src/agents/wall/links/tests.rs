use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;

use super::*;

/// A granted folder and a folder beside it, outside it, removed after the test.
struct Folders {
    base: PathBuf,
    root: PathBuf,
    outside: PathBuf,
}

impl Folders {
    fn new() -> Folders {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "ilo-walk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let base = std::env::temp_dir().join(name);
        fs::create_dir_all(base.join("folder")).unwrap();
        fs::create_dir_all(base.join("outside")).unwrap();
        // Resolved, as the folder panel resolves the user's pick.
        let base = base.canonicalize().unwrap();
        Folders {
            root: base.join("folder"),
            outside: base.join("outside"),
            base,
        }
    }

    fn at(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    fn dir(&self, path: &str) {
        fs::create_dir_all(self.at(path)).unwrap();
    }

    fn file(&self, path: &str) {
        fs::write(self.at(path), "").unwrap();
    }

    fn link(&self, path: &str, target: impl AsRef<Path>) {
        symlink(target, self.at(path)).unwrap();
    }

    /// The wall of a task that started now: what's made before this is the user's, after it the
    /// task's.
    fn start(&self) -> Wall {
        sleep(Duration::from_millis(30));
        let wall = Wall {
            folder: Some(self.root.clone()),
            started: SystemTime::now(),
            launcher: None,
            tmp: None,
            place: None,
        };
        sleep(Duration::from_millis(30));
        wall
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.at("locked"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// The clock exactly, since these links are made tens of milliseconds either side of the start.
fn exact(_: &OwnedFd) -> Option<Duration> {
    Some(Duration::ZERO)
}

/// A filesystem whose change times can't be trusted.
fn untrusted(_: &OwnedFd) -> Option<Duration> {
    None
}

fn walk(wall: &Wall) -> Walked {
    outside_with(wall, LIMITS, exact)
}

fn walk_to(wall: &Wall, limits: Limits) -> Walked {
    outside_with(wall, limits, exact)
}

fn listed(walked: &Walked) -> Vec<&str> {
    let paths = walked.links.iter().map(|link| link.path.to_str().unwrap());
    paths.collect()
}

#[test]
fn a_link_the_task_made_out_of_its_folder_is_listed_and_the_users_own_are_not() {
    let folders = Folders::new();
    folders.dir("sub");
    folders.file("sub/notes.txt");
    folders.link("users", &folders.outside);
    let wall = folders.start();
    folders.link("sub/inside", "notes.txt");
    folders.link("sub/up-and-in", "../sub/notes.txt");
    folders.link("away", &folders.outside);
    folders.link("sub/climbs", "../../outside");
    folders.link("sub/dangling", "../../nowhere/at/all");

    let walked = walk(&wall);
    assert_eq!(
        listed(&walked),
        ["away", "sub/climbs", "sub/dangling"],
        "only the task's links out are listed"
    );
    assert_eq!(walked.links[0].target, folders.outside);
    assert_eq!(walked.links[1].target, Path::new("../../outside"));
    assert_eq!((walked.every_leaving, walked.stopped), (false, None));
}

#[test]
fn a_chain_of_links_is_read_to_its_end() {
    let folders = Folders::new();
    folders.dir("a/b");
    folders.file("a/b/file");
    let wall = folders.start();
    // Each part of the way is a link: in, in, then out.
    folders.link("first", "a/second");
    folders.link("a/second", "b/third");
    folders.link("a/b/third", "../../../outside");
    // A chain that only stays in.
    folders.link("staying", "a/staying");
    folders.link("a/staying", "b/file");
    // A link on the way to a place that climbs out through it.
    folders.link("via", "a/hop/../../../outside");
    folders.link("a/hop", "b");
    // An absolute target into the folder is inside.
    folders.link("absolute-in", folders.at("a/b/file"));

    let walked = walk(&wall);
    assert_eq!(
        listed(&walked),
        ["a/b/third", "a/second", "first", "via"],
        "a chain ending out is listed at each of its links"
    );
}

#[test]
fn a_link_that_loops_is_not_followed_forever() {
    let folders = Folders::new();
    let wall = folders.start();
    folders.link("ping", "pong");
    folders.link("pong", "ping");
    folders.link("self", "self");
    folders.link("through", "ping/../../outside");
    let walked = walk(&wall);
    assert!(walked.links.is_empty(), "{:?}", walked.links);
}

#[test]
fn a_folder_that_is_a_link_is_not_walked_into() {
    let folders = Folders::new();
    folders.dir("sub");
    fs::create_dir(folders.outside.join("deeper")).unwrap();
    let wall = folders.start();
    folders.link("sub/out", "../../outside");
    // A link to a folder inside, and one to the folder outside, which holds links of its own.
    folders.link("again", "sub");
    folders.link("door", &folders.outside);
    symlink("/", folders.outside.join("deeper/root")).unwrap();

    let walked = walk(&wall);
    assert_eq!(
        listed(&walked),
        ["door", "sub/out"],
        "each link once, and nothing past one"
    );
}

#[test]
fn the_walk_stops_short_and_says_so() {
    let folders = Folders::new();
    for n in 0..300 {
        folders.file(&format!("file-{n}"));
    }
    let wall = folders.start();
    folders.link("away", &folders.outside);

    let at_the_cap = walk_to(
        &wall,
        Limits {
            entries: 100,
            ..LIMITS
        },
    );
    assert_eq!(at_the_cap.stopped, Some(Stop::Entries));
    let out_of_time = walk_to(
        &wall,
        Limits {
            time: Duration::ZERO,
            ..LIMITS
        },
    );
    assert_eq!(out_of_time.stopped, Some(Stop::Time));
    let whole = walk(&wall);
    assert_eq!((listed(&whole), whole.stopped), (vec!["away"], None));
}

/// A pipe blocks whoever opens it to read, and a locked file and folder refuse it, so a walk that
/// opened a file would hang or fail here.
#[test]
fn the_walk_opens_no_file() {
    let folders = Folders::new();
    let pipe = c_string(folders.at("pipe").as_os_str()).unwrap();
    // SAFETY: a NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(pipe.as_ptr(), 0o600) }, 0);
    folders.file("secret");
    fs::set_permissions(folders.at("secret"), fs::Permissions::from_mode(0o000)).unwrap();
    let wall = folders.start();
    folders.link("away", &folders.outside);
    folders.link("to-the-pipe", "pipe");
    folders.dir("locked");
    folders.link("locked/away", &folders.outside);
    fs::set_permissions(folders.at("locked"), fs::Permissions::from_mode(0o000)).unwrap();

    let walked = walk(&wall);
    assert!(listed(&walked).contains(&"away"), "{:?}", walked.links);
}

#[test]
fn delete_removes_only_the_listed_links() {
    let folders = Folders::new();
    folders.dir("sub");
    let wall = folders.start();
    folders.link("away", &folders.outside);
    folders.link("sub/also", "../../outside");
    let walked = walk(&wall);
    let only = [walked.links[0].clone()];

    assert_eq!(
        remove_with(&wall, &only, exact),
        Removed {
            removed: 1,
            left: 0
        }
    );
    assert!(fs::symlink_metadata(folders.at("away")).is_err());
    assert!(fs::symlink_metadata(folders.at("sub/also")).is_ok());
    assert!(folders.outside.is_dir(), "the target is never touched");
}

#[test]
fn a_link_changed_since_the_walk_is_left() {
    let folders = Folders::new();
    folders.dir("sub");
    folders.dir("elsewhere");
    folders.file("inside");
    let wall = folders.start();
    for name in ["to-a-file", "retargeted", "gone", "sub/moved"] {
        folders.link(name, &folders.outside);
    }
    let walked = walk(&wall);
    assert_eq!(walked.links.len(), 4);

    fs::remove_file(folders.at("to-a-file")).unwrap();
    folders.file("to-a-file");
    fs::remove_file(folders.at("retargeted")).unwrap();
    folders.link("retargeted", "inside");
    fs::remove_file(folders.at("gone")).unwrap();
    // Its folder swapped for a link to another holding a link of the same name and target.
    fs::rename(folders.at("sub"), folders.at("elsewhere/sub")).unwrap();
    folders.link("sub", "elsewhere/sub");

    assert_eq!(
        remove_with(&wall, &walked.links, exact),
        Removed {
            removed: 0,
            left: 4
        }
    );
    assert!(fs::symlink_metadata(folders.at("elsewhere/sub/moved")).is_ok());
    assert!(folders.at("to-a-file").is_file());
}

#[test]
fn delete_takes_nothing_the_walk_could_not_have_listed() {
    let folders = Folders::new();
    folders.link("users", &folders.outside);
    let wall = folders.start();
    folders.link("away", &folders.outside);
    let link = |path: &str| LeavingLink {
        path: PathBuf::from(path),
        target: folders.outside.clone(),
    };
    let named = [
        // The user's own, from before the task.
        link("users"),
        // Not under the folder.
        link("../folder/away"),
        LeavingLink {
            path: folders.at("away"),
            target: folders.outside.clone(),
        },
        link(""),
    ];
    assert_eq!(
        remove_with(&wall, &named, exact),
        Removed {
            removed: 0,
            left: 4
        }
    );
    assert!(fs::symlink_metadata(folders.at("users")).is_ok());
    assert!(fs::symlink_metadata(folders.at("away")).is_ok());
}

#[test]
fn a_task_with_no_folder_has_no_links() {
    let wall = Wall {
        folder: None,
        started: SystemTime::now(),
        launcher: None,
        tmp: None,
        place: None,
    };
    assert_eq!(walk(&wall), Walked::default());
}

#[test]
fn a_folder_it_cannot_read_is_counted_with_its_path() {
    let folders = Folders::new();
    folders.dir("open");
    let wall = folders.start();
    folders.link("open/away", &folders.outside);
    folders.dir("locked");
    folders.link("locked/away", &folders.outside);
    fs::set_permissions(folders.at("locked"), fs::Permissions::from_mode(0o000)).unwrap();

    let walked = walk(&wall);
    assert_eq!(listed(&walked), ["open/away"]);
    assert_eq!(walked.unread, 1);
    assert_eq!(walked.first_unread.as_deref(), Some(Path::new("locked")));
}

#[test]
fn a_folder_deeper_than_the_walk_goes_is_counted_not_passed_over() {
    let folders = Folders::new();
    folders.dir("a/b/c/d");
    let wall = folders.start();
    folders.link("a/b/c/d/away", &folders.outside);
    folders.link("a/also-deep", &folders.outside);

    // The folder itself, a and b are held open; c would be the fourth.
    let walked = walk_to(&wall, Limits { depth: 3, ..LIMITS });
    assert_eq!(listed(&walked), ["a/also-deep"]);
    assert_eq!(walked.unread, 1);
    assert_eq!(walked.first_unread.as_deref(), Some(Path::new("a/b/c")));
    assert_eq!(listed(&walk(&wall)), ["a/also-deep", "a/b/c/d/away"]);
}

#[test]
fn a_folder_that_cannot_be_opened_is_its_own_answer() {
    let folders = Folders::new();
    let mut wall = folders.start();
    wall.folder = Some(folders.at("removed"));
    let walked = walk(&wall);
    assert!(walked.root_unread);
    assert_eq!((walked.unread, walked.first_unread), (0, None));
}

/// Each way of not seeing the whole folder is an answer of its own, never "nothing found".
#[test]
fn no_give_up_reads_as_a_clean_walk() {
    let folders = Folders::new();
    folders.dir("locked/b");
    folders.file("one");
    folders.file("two");
    let mut wall = folders.start();
    fs::set_permissions(folders.at("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let unread = walk(&wall);
    fs::set_permissions(folders.at("locked"), fs::Permissions::from_mode(0o755)).unwrap();
    let stopped = walk_to(
        &wall,
        Limits {
            entries: 1,
            ..LIMITS
        },
    );
    wall.folder = Some(folders.at("removed"));
    let root_unread = walk(&wall);

    for walked in [unread, stopped, root_unread] {
        assert!(walked.links.is_empty());
        assert_ne!(walked, Walked::default());
    }
}

#[test]
fn a_folder_on_another_device_is_not_walked() {
    let stand_in = |device| {
        // SAFETY: a plain C struct, for which all zeroes is a value.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        stat.st_dev = device;
        stat
    };
    assert!(same_device(&stand_in(7), &stand_in(7)));
    assert!(!same_device(&stand_in(7), &stand_in(8)));
}

#[test]
fn only_the_systems_own_disks_have_change_times_to_trust() {
    assert_eq!(slack_by_name(b"apfs"), Some(Duration::from_millis(100)));
    for name in [
        &b"hfs"[..],
        b"smbfs",
        b"nfs",
        b"msdos",
        b"exfat",
        b"macfuse",
    ] {
        assert_eq!(
            slack_by_name(name),
            None,
            "{}",
            String::from_utf8_lossy(name)
        );
    }
    #[cfg(target_os = "linux")]
    {
        #[allow(clippy::unnecessary_cast)]
        let (ext4, nfs, fuse) = (
            libc::EXT4_SUPER_MAGIC as u32,
            libc::NFS_SUPER_MAGIC as u32,
            libc::FUSE_SUPER_MAGIC as u32,
        );
        assert_eq!(slack_by_number(ext4), Some(Duration::from_secs(1)));
        assert_eq!((slack_by_number(nfs), slack_by_number(fuse)), (None, None));
    }
}

#[test]
fn with_no_change_time_to_trust_every_leaving_link_is_listed_and_delete_takes_none() {
    let folders = Folders::new();
    folders.link("users", &folders.outside);
    let wall = folders.start();
    folders.link("away", &folders.outside);

    let walked = outside_with(&wall, LIMITS, untrusted);
    assert!(walked.every_leaving);
    assert_eq!(listed(&walked), ["away", "users"]);
    assert_eq!(
        remove_with(&wall, &walked.links, untrusted),
        Removed {
            removed: 0,
            left: 2
        }
    );
    assert!(fs::symlink_metadata(folders.at("users")).is_ok());
    assert!(fs::symlink_metadata(folders.at("away")).is_ok());
}
