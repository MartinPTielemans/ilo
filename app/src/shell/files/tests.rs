use std::path::{Path, PathBuf};

use gpui::{TestAppContext, point, px};

use super::{FOLDER_ROWS, Listing, PANEL_PICKS_BOTH, file_path, list};
use crate::shell::{Folders, Page, drag::detents::shell_with, file_url};

/// A folder of its own under the system's temporary one, made empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ilo-files-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `dir` holding "Notes" with "b.txt", "A.txt", a hidden ".cache" and "sub" with "c.txt".
fn notes(dir: &Path) -> PathBuf {
    let notes = dir.join("Notes");
    std::fs::create_dir_all(notes.join("sub")).unwrap();
    for file in ["b.txt", "A.txt", ".cache"] {
        std::fs::write(notes.join(file), "x").unwrap();
    }
    std::fs::write(notes.join("sub").join("c.txt"), "x").unwrap();
    notes
}

/// `path`'s URL as the app makes it: on Windows "file:///C:/…", its backslashes slashes.
fn url(path: &Path) -> String {
    file_url(&path.to_string_lossy())
}

/// A `file:` URL gives back the path it names, its escapes undone; other URLs give none.
#[test]
fn a_file_url_names_its_path() {
    let path = |url: &str| file_path(url).map(|path| path.to_string_lossy().into_owned());
    assert_eq!(path("file:///tmp/my%20photo.jpg").as_deref(), Some("/tmp/my photo.jpg"));
    assert_eq!(path("file://localhost/etc/hosts").as_deref(), Some("/etc/hosts"));
    assert_eq!(path("file:///%E6%97%A5%E6%9C%AC").as_deref(), Some("/日本"));
    assert_eq!(path("file:///100%").as_deref(), Some("/100%"));
    assert_eq!(path("https://example.com/a"), None);
    assert_eq!(path("file:relative"), None);
    let odd = "/tmp/a b#c%d日";
    assert_eq!(path(&file_url(odd)).as_deref(), Some(odd));
    // A drive's, as Windows names it, with backslashes.
    if cfg!(windows) {
        let windows = r"C:\Users\a b\Notes\c.txt";
        assert_eq!(file_url(windows), "file:///C:/Users/a%20b/Notes/c.txt");
        assert_eq!(path(&file_url(windows)).as_deref(), Some(windows));
    }
}

/// A folder comes in by name, case aside, its hidden files left out, its folders listed too, and
/// no more rows than it's given: its own first.
#[test]
fn a_folder_lists_its_files_and_folders_by_name() {
    let dir = scratch("list");
    let notes = notes(&dir);
    let c = url(&notes.join("sub").join("c.txt"));
    let sub = Listing { name: "sub".into(), files: vec![c], ..Default::default() };
    let whole = Listing {
        name: "Notes".into(),
        folders: vec![sub],
        files: vec![url(&notes.join("A.txt")), url(&notes.join("b.txt"))],
    };
    assert_eq!(list(&notes, &mut FOLDER_ROWS.clone(), 0), whole);
    let mut rows = 2;
    let two = list(&notes, &mut rows, 0);
    assert_eq!((two.files.len(), two.folders.len(), rows), (2, 0, 0));
    let mut rows = 3;
    let three = list(&notes, &mut rows, 0);
    assert_eq!((three.files.len(), three.folders.len()), (2, 1));
    assert!(three.folders[0].files.is_empty(), "the folder inside took a row it didn't have");
    std::fs::remove_dir_all(dir).unwrap();
}

/// A folder from the desktop becomes an open folder of the Space at the bottom of its pinned
/// tabs, named for it, its folder closed inside it first, then a tab for each file, shown by none.
#[gpui::test]
fn a_folder_from_the_desktop_becomes_a_folder_of_its_files(cx: &mut TestAppContext) {
    let dir = scratch("drop");
    let notes = notes(&dir);
    let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
    host.take();
    shell.update(cx, |shell, cx| shell.add_folders(vec![notes.clone()], 0, usize::MAX, cx));
    cx.run_until_parked();
    let edits = host.take_edits();
    let made = |edit: &String| edit.split(' ').nth(1).unwrap().parse::<u64>().unwrap();
    assert_eq!(edits.len(), 2, "{edits:?}");
    let (outer, inner) = (made(&edits[0]), made(&edits[1]));
    assert_eq!(edits, [
        format!("folder {outer} 1 Notes 0 {}", usize::MAX),
        format!("folder {inner} 1 sub {outer} 0"),
    ]);
    let calls = host.take();
    let opened = |folder, files: &[&str], index| {
        let urls: Vec<String> = (files.iter())
            .map(|file| url(&file.split('/').fold(notes.clone(), |path, name| path.join(name))))
            .collect();
        format!("open behind {} true {folder} {index}", urls.join(" "))
    };
    assert!(calls.contains(&opened(inner, &["sub/c.txt"], 0)), "{calls:?}");
    assert!(calls.contains(&opened(outer, &["A.txt", "b.txt"], 1)), "{calls:?}");
    assert!(calls.contains(&format!("folder open {inner} false")), "{calls:?}");
    assert!(!calls.contains(&format!("folder open {outer} false")), "{calls:?}");
    assert!(!calls.iter().any(|call| call.starts_with("select")), "a file showed: {calls:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// Picked with the +'s Add File or Folder…, a file opens at the top of Today and a folder comes
/// in at the bottom of the pinned tabs.
#[gpui::test]
fn picked_files_open_and_picked_folders_come_in(cx: &mut TestAppContext) {
    let dir = scratch("pick");
    let notes = notes(&dir);
    let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
    host.take();
    let picked = vec![notes.join("A.txt"), notes.clone()];
    shell.update(cx, |shell, cx| shell.add_paths(picked, cx));
    cx.run_until_parked();
    let calls = host.take();
    assert!(calls.contains(&format!("open {} false 0 0", url(&notes.join("A.txt")))), "{calls:?}");
    let edits = host.take_edits();
    assert!(edits.iter().any(|edit| edit.ends_with(&format!(" 1 Notes 0 {}", usize::MAX))));
    std::fs::remove_dir_all(dir).unwrap();
}

/// A tab showing a file on the desktop is a file's row, to drag onto a page; a folder's, a web
/// page's or a file gone isn't.
#[gpui::test]
fn a_file_tab_is_a_file_row(cx: &mut TestAppContext) {
    let dir = scratch("row");
    let notes = notes(&dir);
    let (shell, _) = shell_with(cx, &[], &[10, 11, 12, 13], Folders::default());
    let file = notes.join("A.txt");
    let pages = [
        (10, url(&file)),
        (11, url(&notes)),
        (12, "https://example.com/".to_string()),
        (13, url(&notes.join("gone.txt"))),
    ];
    for (tab, url) in pages {
        let page = Page { url: url.into(), ..Page::default() };
        shell.update(cx, |shell, cx| shell.set_page(tab, page, cx));
    }
    let rows: Vec<Option<String>> =
        shell.read_with(cx, |shell, _| [10, 11, 12, 13].map(|tab| shell.file_of_row(tab)).to_vec());
    assert_eq!(rows, [Some(file.to_string_lossy().into_owned()), None, None, None]);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The +'s menu ends with what adds from the desktop, after a line: one item where the system's
/// panel picks files and folders together, else one for each.
#[gpui::test]
fn the_plus_menu_adds_files_and_folders(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[1], &[10], Folders::default());
    shell.update(cx, |shell, cx| shell.plus_menu(point(px(200.), px(700.)), cx));
    let menu: Vec<String> =
        host.menus.borrow().last().unwrap().iter().map(|(title, _)| title.to_string()).collect();
    let disk: &[&str] = match PANEL_PICKS_BOTH {
        true => &["", "Add File or Folder…"],
        false => &["", "Add Files…", "Add Folder…"],
    };
    assert_eq!(menu[menu.len() - disk.len()..], *disk);
    host.take();
    let dir = scratch("plus");
    let notes = notes(&dir);
    // SAFETY: the tests that read it are this one alone.
    unsafe { std::env::set_var("BROWSER_ADD_PATHS", notes.join("b.txt")) };
    cx.update(|cx| host.choose(disk[1], cx));
    unsafe { std::env::remove_var("BROWSER_ADD_PATHS") };
    cx.run_until_parked();
    let calls = host.take();
    assert!(calls.contains(&format!("open {} false 0 0", url(&notes.join("b.txt")))), "{calls:?}");
    std::fs::remove_dir_all(dir).unwrap();
}
