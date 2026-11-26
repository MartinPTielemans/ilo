//! Sort Tabs into Folders (#4252) in a window, on a host that records what the shell asks of the
//! core.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::broom::Folder;
use crate::shell::recorder::Recorder;
use crate::shell::{Folders, Page, Shell, Split};

/// Today's tabs: four on the web, a split view (14 and 15), a page off the web (16), and two in a
/// today's folder (17 and 18, folder 50, "Work"). Pinned: 30, in the pinned folder 60, "Trips".
const TABS: &[(u64, &str, &str)] = &[
    (10, "https://github.com/ilo/ilo?tab=readme#top", "ilo"),
    (11, "https://en.wikipedia.org/wiki/Arc", "Arc"),
    (12, "http://news.ycombinator.com/", "Hacker News"),
    (13, "https://mail.example.com/inbox/123", "Your reset link"),
    (14, "https://example.com/a", "A"),
    (15, "https://github.com/x", "X"),
    (16, "about:blank", "Blank"),
    (17, "https://docs.rs/gpui", "gpui"),
    (18, "https://crates.io/", "crates"),
];

fn folders() -> Folders {
    Folders {
        folders: HashMap::from([(50, ("Work".into(), true)), (60, ("Trips".into(), true))]),
        parents: HashMap::from([(17, 50), (18, 50), (30, 60)]),
        ..Folders::default()
    }
}

fn shell<'a>(
    cx: &'a mut TestAppContext,
    agent: &str,
) -> (Entity<Shell>, &'a mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (
            1,
            "Space".into(),
            None,
            1,
            "".into(),
            agent.into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let today = TABS.iter().map(|&(tab, ..)| tab).collect();
        let split = vec![Split {
            tabs: vec![14, 15],
            ..Default::default()
        }];
        shell.set_tabs(1, vec![60, 30], today, split, folders(), cx);
        for &(tab, url, title) in TABS {
            shell.set_page(
                tab,
                Page {
                    url: url.into(),
                    ..Page::default()
                },
                cx,
            );
            shell.set_title(tab, title.into(), cx);
        }
    });
    cx.run_until_parked();
    host.take();
    host.take_edits();
    (shell, cx, host)
}

fn folder(name: &str, tabs: &[usize]) -> Folder {
    Folder {
        name: name.into(),
        tabs: tabs.to_vec(),
    }
}

#[gpui::test]
fn a_sort_hands_loose_web_tabs_alone_each_as_its_title_and_host(cx: &mut TestAppContext) {
    let (shell, cx, _) = shell(cx, "");
    let handed = shell.read_with(cx, |shell, _| shell.broom_tabs(1));
    let handed: Vec<(u64, &str, &str)> = handed
        .iter()
        .map(|(tab, each)| (*tab, each.title.as_str(), each.host.as_str()))
        .collect();
    assert_eq!(
        handed,
        [
            (10, "ilo", "github.com"),
            (11, "Arc", "en.wikipedia.org"),
            (12, "Hacker News", "news.ycombinator.com"),
            (13, "Your reset link", "mail.example.com"),
        ],
        "no path, query or fragment; no split, page off the web, foldered or pinned tab"
    );
    assert!(shell.read_with(cx, |shell, _| shell.broom_sorts(1)));
}

#[gpui::test]
fn a_sort_needs_four_tabs_that_count(cx: &mut TestAppContext) {
    let (shell, cx, _) = shell(cx, "");
    let off_web = |tab: u64, shell: &mut Shell, cx: &mut gpui::Context<Shell>| {
        let page = Page {
            url: "about:blank".into(),
            ..Page::default()
        };
        shell.set_page(tab, page, cx)
    };
    shell.update(cx, |shell, cx| {
        off_web(10, shell, cx);
    });
    let sorts = |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.broom_sorts(1));
    assert!(!sorts(cx), "three tabs left that count");
    assert!(shell.read_with(cx, |shell, _| shell.broom_counts(13)
        && !shell.broom_counts(10)));
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://example.org/".into(),
            ..Page::default()
        };
        shell.set_page(16, page, cx)
    });
    assert!(sorts(cx), "a page that turned web counts");
    assert!(
        !shell.read_with(cx, |shell, _| shell.broom_sorts(2)),
        "no such Space"
    );
}

#[gpui::test]
fn an_agents_space_hands_nothing(cx: &mut TestAppContext) {
    let (shell, cx, _) = shell(cx, "Claude Code");
    assert!(!shell.read_with(cx, |shell, _| shell.broom_sorts(1)));
    assert!(
        shell
            .read_with(cx, |shell, _| shell.broom_tabs(1))
            .is_empty()
    );
}

#[gpui::test]
fn a_sort_makes_its_folders_and_joins_a_todays_folder_of_its_name(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx, "");
    host.today_folders_from.set(70);
    let handed = [10, 11, 12, 13];
    let answer = vec![folder("work", &[0, 3]), folder("Trips", &[1, 2])];
    let sorted = shell.update(cx, |shell, cx| {
        shell.sort_into_folders(1, &handed, answer, cx)
    });
    assert_eq!(sorted, Some((4, 2)));
    assert_eq!(
        host.take_edits(),
        [
            "today folder Trips in 1 of [11, 12]",
            "join [10, 13] to today folder 50",
        ],
        "a pinned folder's name makes a new one; a today's folder's, case aside, is joined"
    );
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    // The today's folder Work, 17 and 18, is one row: it goes back to its place whole.
    assert_eq!(
        host.take_edits(),
        ["unsort 1 made [70] joined [10, 13] rows [10, 11, 12, 13, 14, 16, 50]"],
        "one step: the new folder goes, the joined tabs leave, the order comes back"
    );
    // The answer isn't kept, so there's nothing to redo.
    assert!(!shell.update(cx, |shell, cx| shell.redo(cx)));
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn a_sort_the_core_makes_no_folders_for_moves_nothing_and_keeps_no_step(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx, "");
    let answer = vec![folder("Reading", &[0, 1])];
    let sorted = shell.update(cx, |shell, cx| {
        shell.sort_into_folders(1, &[10, 11], answer, cx)
    });
    assert_eq!(sorted, None);
    host.take_edits();
    assert!(!shell.update(cx, |shell, cx| shell.undo(cx)));
}

#[gpui::test]
fn a_tab_that_changed_during_the_run_moves_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx, "");
    host.today_folders_from.set(70);
    shell.update(cx, |shell, cx| {
        shell.set_page(
            11,
            Page {
                url: "about:blank".into(),
                ..Page::default()
            },
            cx,
        )
    });
    let answer = vec![folder("Reading", &[0, 1])];
    let sorted = shell.update(cx, |shell, cx| {
        shell.sort_into_folders(1, &[10, 11], answer, cx)
    });
    assert_eq!(sorted, None);
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn a_ninth_new_folder_moves_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx, "");
    host.today_folders_from.set(70);
    let answer: Vec<Folder> = (0..9)
        .map(|at| folder(&format!("F{at}"), &[at, at]))
        .collect();
    let sorted = shell.update(cx, |shell, cx| {
        shell.sort_into_folders(1, &[10, 11], answer, cx)
    });
    assert_eq!(sorted, None);
    assert!(host.take_edits().is_empty());
}

/// The pass's own cost, as ⌘T and a window's first showing of a Space run it: 200 today's tabs
/// none of which counts, which it reads to the end, and the usual case, where the first four count.
/// `cargo test --release --lib broom_pass_bench -- --ignored --nocapture`
#[gpui::test]
#[ignore]
fn broom_pass_bench(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    let today: Vec<u64> = (100..300).collect();
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            today.clone(),
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    let time = |url: &str, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            for &tab in &today {
                let page = Page {
                    url: url.to_string().into(),
                    ..Page::default()
                };
                shell.set_page(tab, page, cx);
            }
        });
        shell.read_with(cx, |shell, _| {
            const PASSES: u32 = 10_000;
            let started = std::time::Instant::now();
            let mut sorts = 0;
            for _ in 0..PASSES {
                sorts += usize::from(std::hint::black_box(shell.broom_sorts(1)));
            }
            (started.elapsed() / PASSES, sorts)
        })
    };
    let (worst, none) = time("about:blank", &shell, cx);
    let (usual, all) = time("https://example.com/", &shell, cx);
    assert_eq!((none, all), (0, 10_000));
    println!("broom pass: 200 tabs none counting {worst:?}, the usual case {usual:?}");
}
