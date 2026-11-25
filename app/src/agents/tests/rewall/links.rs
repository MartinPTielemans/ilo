//! The check for links a walled run left leading out of its folder, as the task's lines say it
//! (X10, X11) and as the user acts on them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::agents::task::TaskAction;
use crate::agents::task::ilo::{IloButton, Unchecked, buttons};
use crate::agents::wall::{self, Found, LeavingLink, Walked};

/// A link the check lists, by its path in the folder.
fn link(path: &str) -> LeavingLink {
    LeavingLink {
        path: path.into(),
        target: "/elsewhere/taxes".into(),
    }
}

/// What the check found in the folder `Granted::for_test` grants.
fn found(walked: Walked) -> Found {
    let wall = Wall::granted_for_test(Granted::for_test());
    Found {
        walked: Arc::new(walked),
        wall: Arc::new(wall),
    }
}

/// The task's lines of the check, in order, each with its key and whether it's settled.
fn checked(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<IloLine> {
    (run.talk(agent, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Ilo(line)
                if matches!(
                    line.kind,
                    IloKind::LinksOut { .. } | IloKind::LinksUnchecked { .. }
                ) =>
            {
                Some(line)
            }
            _ => None,
        })
        .collect()
}

fn revealed(run: &Run, agent: &str, line: &IloLine, cx: &mut TestAppContext) -> Option<PathBuf> {
    (run.agents).read_with(cx, |agents, _| agents.link_to_reveal(agent, line.key))
}

fn act(run: &Run, agent: &str, action: TaskAction, cx: &mut TestAppContext) {
    (run.agents).update(cx, |agents, cx| _ = agents.task_action(agent, action, cx));
    cx.run_until_parked();
}

/// A task ended with its folder, whose check found links in it: X10, by its count and the
/// folder's name, with Show in Finder on the first and Delete Links, which stay on the ended
/// task until the user writes again.
#[gpui::test]
fn links_left_leading_out_are_a_line_with_show_and_delete(cx: &mut TestAppContext) {
    let (run, _, agent) = started("links-out", folder(), cx);
    run.ran(1);
    run.finish(&agent, cx);
    let ended = (run.agents).read_with(cx, |agents, _| agents.task(&agent).unwrap().ended);
    assert!(ended.is_some());
    let walked = Walked {
        links: vec![link("notes/out"), link("out")],
        ..Walked::default()
    };
    run.said(&agent, Out::Links(found(walked)), cx);
    let lines = checked(&run, &agent, cx);
    let [line] = lines.as_slice() else {
        panic!("{lines:?}");
    };
    let out = IloKind::LinksOut {
        folder: FOLDER.into(),
        count: 2,
        every: false,
    };
    assert_eq!((&line.kind, line.settled), (&out, false));
    assert_eq!(
        buttons(line),
        [IloButton::RevealLink, IloButton::DeleteLinks(2)]
    );
    let first = Path::new("/nonexistent/folder/notes/out");
    assert_eq!(revealed(&run, &agent, line, cx).as_deref(), Some(first));
    // The user writes again: it's passed.
    (run.agents).update(cx, |agents, cx| agents.reply(&agent, "And on Sunday?", cx));
    assert!(checked(&run, &agent, cx).iter().all(|line| line.settled));
}

/// Where the folder keeps no change time to trust, the list is every leaving link, the user's
/// own too: shown, never deleted, even by a press that comes anyway.
#[gpui::test]
fn every_leaving_link_is_shown_and_never_deleted(cx: &mut TestAppContext) {
    let (run, _, agent) = started("links-every", folder(), cx);
    run.ran(1);
    let walked = Walked {
        links: vec![link("out")],
        every_leaving: true,
        ..Walked::default()
    };
    run.said(&agent, Out::Links(found(walked)), cx);
    let [line] = checked(&run, &agent, cx).try_into().unwrap();
    let out = IloKind::LinksOut {
        folder: FOLDER.into(),
        count: 1,
        every: true,
    };
    assert_eq!(line.kind, out);
    assert_eq!(buttons(&line), [IloButton::RevealLink]);
    act(&run, &agent, TaskAction::DeleteLinks(line.key), cx);
    assert!(!checked(&run, &agent, cx)[0].settled);
}

/// What the check couldn't read is X11, one form, the widest first: the folder itself, then a
/// walk cut short, which can't count what it missed, then the folders inside it couldn't read.
/// Each shows where it couldn't read, but a walk cut short, which missed no one place.
#[gpui::test]
fn what_the_check_couldnt_read_is_one_line_the_widest_first(cx: &mut TestAppContext) {
    let (run, _, agent) = started("links-unread", folder(), cx);
    run.ran(1);
    let deep = || Some(PathBuf::from("deep/er"));
    let cases = [
        (
            Walked {
                root_unread: true,
                stopped: Some(wall::Stop::Time),
                unread: 2,
                first_unread: deep(),
                ..Walked::default()
            },
            Unchecked::Folder,
            Some("/nonexistent/folder"),
        ),
        (
            Walked {
                stopped: Some(wall::Stop::Entries),
                unread: 2,
                first_unread: deep(),
                ..Walked::default()
            },
            Unchecked::Part,
            None,
        ),
        (
            Walked {
                unread: 1,
                first_unread: deep(),
                ..Walked::default()
            },
            Unchecked::Inside(1),
            Some("/nonexistent/folder/deep/er"),
        ),
        (
            Walked {
                unread: 3,
                first_unread: deep(),
                ..Walked::default()
            },
            Unchecked::Inside(3),
            Some("/nonexistent/folder/deep/er"),
        ),
    ];
    for (walked, unchecked, shows) in cases {
        run.said(&agent, Out::Links(found(walked)), cx);
        let line = checked(&run, &agent, cx).pop().unwrap();
        let kind = IloKind::LinksUnchecked {
            folder: FOLDER.into(),
            unchecked,
        };
        assert_eq!((&line.kind, line.settled), (&kind, false));
        let shown = revealed(&run, &agent, &line, cx);
        assert_eq!(shown.as_deref(), shows.map(Path::new), "{unchecked:?}");
        let offered = buttons(&line);
        assert_eq!(offered.is_empty(), shows.is_none(), "{unchecked:?}");
    }
    assert_eq!(checked(&run, &agent, cx).len(), 4);
}

/// Links found by a walk that couldn't read it all: X10, then X11 under it. A clean walk says
/// nothing, and a task that's gone takes none.
#[gpui::test]
fn what_couldnt_be_read_stands_under_the_links_and_a_clean_walk_says_nothing(
    cx: &mut TestAppContext,
) {
    let (run, _, agent) = started("links-both", folder(), cx);
    run.ran(1);
    run.said(&agent, Out::Links(found(Walked::default())), cx);
    assert_eq!(checked(&run, &agent, cx), []);
    let walked = Walked {
        links: vec![link("out")],
        unread: 2,
        first_unread: Some("deep".into()),
        ..Walked::default()
    };
    run.said(&agent, Out::Links(found(walked)), cx);
    let kinds: Vec<IloKind> = (checked(&run, &agent, cx).into_iter())
        .map(|line| line.kind)
        .collect();
    let out = IloKind::LinksOut {
        folder: FOLDER.into(),
        count: 1,
        every: false,
    };
    let unread = IloKind::LinksUnchecked {
        folder: FOLDER.into(),
        unchecked: Unchecked::Inside(2),
    };
    assert_eq!(kinds, [out, unread]);

    let gone = runner::Event {
        agent: "gone".into(),
        run: 1,
        out: Out::Links(found(Walked {
            links: vec![link("out")],
            ..Walked::default()
        })),
    };
    (run.agents).update(cx, |agents, cx| agents.task_said(gone, cx));
    let any = (run.agents).read_with(cx, |agents, _| agents.task("gone").is_some());
    assert!(!any);
}

/// Delete Links removes exactly the links its line lists, in the folder itself, and settles the
/// line; nothing of where they are reaches the conversation's text or the activity log.
#[cfg(unix)]
#[gpui::test]
fn delete_links_removes_exactly_the_listed_links(cx: &mut TestAppContext) {
    const NAME: &str = "Saffron-link";
    let base = std::env::temp_dir().join(format!("ilo-links-delete-{}", std::process::id()));
    std::fs::create_dir_all(base.join("folder")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    let base = base.canonicalize().unwrap();
    let (root, outside) = (base.join("folder"), base.join("outside"));
    let wall = Wall::granted_for_test(Granted::picked_for_test(root.clone()));
    let made = |name: &str| {
        std::os::unix::fs::symlink(&outside, root.join(name)).unwrap();
        LeavingLink {
            path: name.into(),
            target: outside.clone(),
        }
    };
    let listed = vec![made(NAME), made("second")];
    made("unlisted");

    let (run, _, agent) = started("links-delete", folder(), cx);
    run.ran(1);
    let log = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| {
            format!("{:?}", agents.recent().collect::<Vec<_>>())
        })
    };
    let before = log(cx);
    let walked = Walked {
        links: listed,
        ..Walked::default()
    };
    let found = Found {
        walked: Arc::new(walked),
        wall: Arc::new(wall),
    };
    run.said(&agent, Out::Links(found), cx);
    let [line] = checked(&run, &agent, cx).try_into().unwrap();
    act(&run, &agent, TaskAction::DeleteLinks(line.key), cx);
    let left = |name: &str| root.join(name).symlink_metadata().is_ok();
    assert!(!left(NAME) && !left("second"), "a listed link is left");
    assert!(left("unlisted"), "a link it didn't list went");
    assert!(outside.is_dir(), "a link's target went");
    assert!(checked(&run, &agent, cx)[0].settled);
    // A second press deletes nothing more.
    std::os::unix::fs::symlink(&outside, root.join(NAME)).unwrap();
    act(&run, &agent, TaskAction::DeleteLinks(line.key), cx);
    assert!(left(NAME));

    let said = format!("{:?}", run.talk(&agent, cx).lines);
    for place in [NAME, "second", base.to_str().unwrap()] {
        assert!(!said.contains(place), "the conversation's text has {place}");
    }
    assert_eq!(log(cx), before, "the activity log changed");
    _ = std::fs::remove_dir_all(&base);
}
