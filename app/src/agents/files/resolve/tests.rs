use std::os::unix::fs::{MetadataExt, symlink};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

/// A tree of the test's own, by its real name (the Mac's temporary folder is behind a link).
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        let base = std::env::temp_dir().join(format!("ilo-resolve-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        Tree(std::fs::canonicalize(&base).unwrap())
    }

    fn at(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }

    fn file(&self, path: &str, text: &str) -> PathBuf {
        let path = self.at(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn link(&self, path: &str, to: impl AsRef<Path>) {
        symlink(to, self.at(path)).unwrap();
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read(path: &Path) -> Result<Resolved, Unresolved> {
    resolve(path, For::Read, &NeverIds::default())
}

/// What the held end holds, read by its handle.
fn text_of(resolved: &Resolved) -> String {
    let held = resolved.target.as_ref().expect("something is there");
    let mut file = std::fs::File::from(held.fd.try_clone().unwrap());
    let mut text = String::new();
    io::Read::read_to_string(&mut file, &mut text).unwrap();
    text
}

fn id_of(path: &Path) -> Id {
    let found = std::fs::symlink_metadata(path).unwrap();
    Id {
        device: found.dev(),
        inode: found.ino(),
    }
}

#[test]
fn a_plain_path_is_its_own_real_path_and_held() {
    let tree = Tree::new("plain");
    let file = tree.file("a/b/c/plan.md", "plan");
    let resolved = read(&file).unwrap();
    assert_eq!(resolved.real, file);
    assert!(!resolved.through_link && !resolved.never);
    assert_eq!(resolved.name, "plan.md");
    assert_eq!(text_of(&resolved), "plan");
    let held = resolved.target.as_ref().unwrap();
    assert_eq!(
        (held.kind, held.id, held.other_names),
        (Kind::File, id_of(&file), false)
    );
    assert!(resolved.still_named());
}

#[test]
fn a_link_at_any_name_is_followed_and_the_real_path_names_where_it_lands() {
    let tree = Tree::new("links");
    let file = tree.file("a/b/c/plan.md", "plan");
    tree.link("first", tree.at("a"));
    tree.link("a/b/middle", "c");
    tree.link("last", "a/b/c/plan.md");
    tree.link("a/b/c/up", "../../..");
    for path in [
        "first/b/c/plan.md",
        "a/b/middle/plan.md",
        "last",
        "a/b/c/up/a/b/c/plan.md",
    ] {
        let resolved = read(&tree.at(path)).unwrap();
        assert_eq!(resolved.real, file, "{path}");
        assert!(resolved.through_link, "{path}");
        assert_eq!(text_of(&resolved), "plan", "{path}");
    }
}

#[test]
fn a_name_nothing_has_yet_is_its_folders_alone() {
    let tree = Tree::new("new");
    tree.file("a/old.md", "");
    tree.link("dangling", "a/new.md");
    for path in ["a/new.md", "dangling"] {
        let resolved = read(&tree.at(path)).unwrap();
        assert_eq!(resolved.real, tree.at("a/new.md"), "{path}");
        assert!(resolved.target.is_none(), "{path}");
        assert!(resolved.still_named(), "{path}");
    }
    assert_eq!(read(&tree.at("gone/new.md")).unwrap_err(), Unresolved::Gone);
    assert_eq!(
        read(&tree.at("a/old.md/x")).unwrap_err(),
        Unresolved::NotAFolder
    );
    assert_eq!(
        read(Path::new("a/old.md")).unwrap_err(),
        Unresolved::Relative
    );
}

#[test]
fn a_loop_and_the_33rd_link_are_refused() {
    let tree = Tree::new("loop");
    tree.file("plan.md", "plan");
    tree.link("one", "two");
    tree.link("two", "one");
    assert_eq!(read(&tree.at("one")).unwrap_err(), Unresolved::Loop);
    // A chain of links, each to the next, the last to the file.
    let chain = |count: usize| {
        let name = |n: usize| format!("chain-{count}-{n}");
        for n in 0..count {
            let to = match n + 1 == count {
                true => "plan.md".to_string(),
                false => name(n + 1),
            };
            tree.link(&name(n), to);
        }
        read(&tree.at(&name(0)))
    };
    assert_eq!(chain(MOST_LINKS).unwrap().real, tree.at("plan.md"));
    assert_eq!(chain(MOST_LINKS + 1).unwrap_err(), Unresolved::Loop);
}

#[test]
fn a_climb_out_stops_at_the_root_and_follows_folders_not_names() {
    let tree = Tree::new("climb");
    let file = tree.file("a/plan.md", "plan");
    let ups = "../".repeat(64);
    let climbed = PathBuf::from(format!("/{ups}{}", file.display()));
    assert_eq!(read(&climbed).unwrap().real, file);
    let root = read(&PathBuf::from(format!("/{ups}"))).unwrap();
    assert_eq!(root.real, Path::new("/"));
    assert_eq!(root.target.as_ref().unwrap().kind, Kind::Folder);
    // `..` after a link climbs from where the link landed, as the system's own walk does.
    tree.file("far/away/note.md", "far");
    tree.link("a/to-away", tree.at("far/away"));
    let back = read(&tree.at("a/to-away/../away/note.md")).unwrap();
    assert_eq!(back.real, tree.at("far/away/note.md"));
}

#[test]
fn a_file_with_another_name_says_so() {
    let tree = Tree::new("hard");
    let file = tree.file("secret.txt", "secret");
    std::fs::hard_link(&file, tree.at("other.txt")).unwrap();
    let resolved = read(&tree.at("other.txt")).unwrap();
    assert!(resolved.target.as_ref().unwrap().other_names);
    assert_eq!(resolved.target.as_ref().unwrap().id, id_of(&file));
}

#[test]
fn a_pipe_is_never_opened() {
    let tree = Tree::new("pipe");
    let pipe = c_string(tree.at("pipe").as_os_str()).unwrap();
    // SAFETY: a NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(pipe.as_ptr(), 0o600) }, 0);
    assert_eq!(read(&tree.at("pipe")).unwrap_err(), Unresolved::NotAFile);
}

#[test]
fn a_change_opens_the_file_to_write_and_a_folder_as_a_folder() {
    let tree = Tree::new("change");
    let file = tree.file("a/plan.md", "plan");
    let never = NeverIds::default();
    let resolved = resolve(&file, For::Change, &never).unwrap();
    let held = resolved.target.as_ref().unwrap();
    // SAFETY: an open handle and a buffer of the length given.
    let wrote = unsafe { libc::pwrite(held.fd.as_raw_fd(), c"PLAN".as_ptr().cast(), 4, 0) };
    assert_eq!(wrote, 4);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "PLAN");
    let folder = resolve(&tree.at("a"), For::Change, &never).unwrap();
    assert_eq!(folder.target.unwrap().kind, Kind::Folder);
}

#[test]
fn a_never_root_is_known_by_its_identity_whatever_name_reaches_it() {
    let tree = Tree::new("never");
    tree.file("keys/id", "key");
    tree.file("work/plan.md", "plan");
    tree.link("work/keys", tree.at("keys"));
    let never = NeverIds::of(&[tree.at("keys")]);
    let through = resolve(&tree.at("work/keys/id"), For::Read, &never).unwrap();
    assert!(through.never);
    let own = resolve(&tree.at("work/plan.md"), For::Read, &never).unwrap();
    assert!(!own.never);
    // The folder itself, and a never root given by a link, which is known by where it leads.
    assert!(resolve(&tree.at("keys"), For::Read, &never).unwrap().never);
    let by_link = NeverIds::of(&[tree.at("work/keys")]);
    assert!(
        resolve(&tree.at("keys/id"), For::Read, &by_link)
            .unwrap()
            .never
    );
    // On a disk whose names are the same whatever their case, as the Mac's is by default.
    if std::fs::metadata(tree.at("KEYS")).is_ok() {
        assert!(
            resolve(&tree.at("KEYS/id"), For::Read, &never)
                .unwrap()
                .never
        );
    }
}

#[test]
fn a_file_put_in_its_place_after_the_resolve_is_not_the_one_held() {
    let tree = Tree::new("swapped");
    let file = tree.file("plan.md", "plan");
    let resolved = read(&file).unwrap();
    tree.file("other.md", "other");
    std::fs::rename(tree.at("other.md"), &file).unwrap();
    assert!(!resolved.still_named());
    assert_eq!(text_of(&resolved), "plan");
    // And a name made where nothing was.
    let new = read(&tree.at("new.md")).unwrap();
    tree.file("new.md", "made");
    assert!(!new.still_named());
}

#[test]
fn a_parent_moved_away_keeps_its_handle_and_its_file() {
    let tree = Tree::new("parent");
    tree.file("trip/plan.md", "plan");
    let resolved = read(&tree.at("trip/plan.md")).unwrap();
    std::fs::rename(tree.at("trip"), tree.at("moved")).unwrap();
    tree.file("trip/plan.md", "planted");
    assert!(resolved.still_named());
    assert_eq!(text_of(&resolved), "plan");
}

/// A thread swaps a folder on the path for a link to another, back and forth, while the resolve
/// runs: each answer's real path is the one the held handle reads, never the one swapped in.
#[test]
fn a_folder_swapped_for_a_link_as_it_resolves_is_named_as_held() {
    let tree = Tree::new("race");
    tree.file("inside/plan.md", "inside");
    tree.file("outside/plan.md", "outside");
    std::fs::create_dir(tree.at("dir")).unwrap();
    std::fs::rename(tree.at("inside"), tree.at("x")).unwrap();
    tree.link("link", tree.at("outside"));
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, x, link, dir) = (stop.clone(), tree.at("x"), tree.at("link"), tree.at("dir"));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                _ = std::fs::rename(&x, &dir);
                _ = std::fs::rename(&link, &x);
                _ = std::fs::rename(&x, &link);
                _ = std::fs::rename(&dir, &x);
            }
        })
    };
    let (mut inside, mut outside) = (0, 0);
    for _ in 0..10_000 {
        let Ok(resolved) = read(&tree.at("x/plan.md")) else {
            continue;
        };
        let Some(_) = resolved.target else {
            continue;
        };
        match resolved.through_link {
            false => {
                assert_eq!(resolved.real, tree.at("x/plan.md"));
                assert_eq!(text_of(&resolved), "inside");
                inside += 1;
            }
            true => {
                assert_eq!(resolved.real, tree.at("outside/plan.md"));
                assert_eq!(text_of(&resolved), "outside");
                outside += 1;
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    assert!(inside + outside > 0, "nothing resolved");
}

/// A link in a granted folder that leads out of it is judged where it lands: outside the folder,
/// and never when it lands on the never list, by name or by identity. Not on the Mac, whose
/// temporary folder is under /private, which is on the never list itself.
#[cfg(not(target_os = "macos"))]
#[test]
fn a_link_out_of_a_granted_folder_is_judged_where_it_lands() {
    use crate::agents::files::{At, Listed, Never, Row};
    let tree = Tree::new("link-out");
    let home = tree.at("home");
    tree.file("home/code/site/index.html", "site");
    tree.file("home/notes.md", "notes");
    tree.file("home/.ssh/id_ed25519", "key");
    tree.link("home/code/site/notes", tree.at("home/notes.md"));
    tree.link("home/code/site/keys", "../../.ssh");
    let never = Never::of(home.clone(), Vec::new(), None);
    let ids = NeverIds::of(&never.by_identity());
    let site = [Listed::for_test(home.join("code/site"), Row::Change)];
    let at = |path: &str| {
        let resolved = resolve(&tree.at(path), For::Read, &ids).unwrap();
        match resolved.never {
            true => At::Never,
            false => never.at(&resolved.real, &site, None),
        }
    };
    assert_eq!(at("home/code/site/index.html"), At::Folder(Row::Change));
    assert_eq!(at("home/code/site/notes"), At::Outside);
    assert_eq!(at("home/code/site/keys/id_ed25519"), At::Never);
}

/// A link in a listed folder that leads into `~/Library`'s credential floor, or into a dot-entry
/// of the home's whatever its name, is never reached, at whichever row the folder is listed: the
/// floor judges the path each access resolves to, never the one named. Not on the Mac, for the
/// reason above.
#[cfg(not(target_os = "macos"))]
#[test]
fn a_link_out_of_a_listed_folder_into_the_library_or_a_dot_entry_is_never_reached() {
    use crate::agents::files::{At, Listed, Never, Row};
    let tree = Tree::new("link-floor");
    let home = tree.at("home");
    tree.file("home/code/site/index.html", "site");
    tree.file("home/Library/Keychains/login.keychain-db", "keys");
    tree.file("home/Library/Application Support/x/Cookies", "cookies");
    tree.file("home/.a-new-tool/token", "token");
    tree.link("home/code/site/keys", tree.at("home/Library/Keychains"));
    tree.link(
        "home/code/site/cookies",
        "../../Library/Application Support/x/Cookies",
    );
    tree.link("home/code/site/token", tree.at("home/.a-new-tool/token"));
    let never = Never::of(home.clone(), Vec::new(), None);
    let ids = NeverIds::of(&never.by_identity());
    for row in [Row::Change, Row::Read] {
        let site = [Listed::for_test(home.join("code/site"), row)];
        let at = |path: &str| {
            let resolved = resolve(&tree.at(path), For::Read, &ids).unwrap();
            match resolved.never {
                true => At::Never,
                false => never.at(&resolved.real, &site, None),
            }
        };
        assert_eq!(at("home/code/site/index.html"), At::Folder(row));
        for path in [
            "home/code/site/keys/login.keychain-db",
            "home/code/site/cookies",
            "home/code/site/token",
        ] {
            assert_eq!(at(path), At::Never, "{path} at {row:?}");
        }
    }
}

/// The Mac's /tmp, /var and /etc are links into /private: the floor resolves them to their real
/// names, which the never list refuses.
#[cfg(target_os = "macos")]
#[test]
fn the_macs_short_system_names_resolve_into_private_and_are_never_reached() {
    use crate::agents::files::{At, Never};
    let never = Never::here(None);
    let ids = NeverIds::of(&never.by_identity());
    for (path, real) in [
        ("/etc/hosts", "/private/etc/hosts"),
        ("/tmp", "/private/tmp"),
        ("/var/db", "/private/var/db"),
    ] {
        let resolved = resolve(Path::new(path), For::Read, &ids).unwrap();
        assert_eq!(resolved.real, Path::new(real), "{path}");
        let at = match resolved.never {
            true => At::Never,
            false => never.at(&resolved.real, &[], None),
        };
        assert_eq!(at, At::Never, "{path}");
    }
}
