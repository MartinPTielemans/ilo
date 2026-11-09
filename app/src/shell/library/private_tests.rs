//! A private window's Library (#669): its own session's downloads and their media, never the
//! regular windows' Spaces, archived tabs, captures or downloads, and theirs never its.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{Captured, Download, DownloadState};
use crate::shell::{Command, Section, Shell, recorder::Recorder};

/// A download of `profile`, in its private session with `private`.
fn download(id: u64, name: &str, profile: u64, private: bool, state: DownloadState) -> Download {
    Download {
        id,
        name: name.to_string().into(),
        url: "https://example.org/".into(),
        path: format!("/tmp/{name}").into(),
        received: 0,
        total: 0,
        state,
        open_when_done: false,
        held: false,
        started: id as f64,
        profile,
        private,
    }
}

/// Every session's downloads, as the core reports them: profile 1's own, its private session's,
/// one done and one coming in, and profile 2's private session's.
fn every_session() -> Vec<Download> {
    vec![
        download(1, "report.pdf", 1, false, DownloadState::Done),
        download(2, "secret.pdf", 1, true, DownloadState::Done),
        download(3, "coming.zip", 1, true, DownloadState::Running),
        download(4, "other.pdf", 2, true, DownloadState::Done),
    ]
}

fn window(
    private: bool,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| match private {
        true => Shell::private_window(browser, 9, "Incognito".into(), 1, cx),
        false => Shell::new(browser, cx),
    });
    shell.update(cx, |shell, cx| shell.set_downloads(every_session(), cx));
    cx.run_until_parked();
    (shell, cx, host)
}

fn listed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<u64> {
    shell.read_with(cx, |shell, _| {
        shell.downloads.iter().map(|d| d.id).collect()
    })
}

/// A private window lists only its own session's downloads, the finished and the coming, and a
/// regular window only the profiles' own, so neither sees the other's.
#[gpui::test]
fn each_window_lists_only_its_sessions_downloads(cx: &mut TestAppContext) {
    let (private, cx, _) = window(true, cx);
    assert_eq!(listed(&private, cx), [2, 3]);
}

#[gpui::test]
fn a_regular_window_never_lists_a_private_sessions_downloads(cx: &mut TestAppContext) {
    let (regular, cx, _) = window(false, cx);
    assert_eq!(listed(&regular, cx), [1]);
}

/// ⇧⌘L and ⇧⌘J open a private window's Library on its Downloads, and it offers Media and
/// Downloads alone: View Archive and the Spaces open nothing, and the regular windows' captures
/// never reach its Media.
#[gpui::test]
fn a_private_library_offers_media_and_downloads_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(true, cx);
    shell.update(cx, |shell, cx| shell.run(Command::ViewLibrary, "test", cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert!(
            shell.library_open,
            "⇧⌘L opened no Library in a private window"
        );
        assert_eq!(shell.section, Section::Downloads);
        assert!(shell.has_section(Section::Media));
        assert!(!shell.has_section(Section::Spaces));
        assert!(!shell.has_section(Section::Archive));
    });
    shell.update(cx, |shell, cx| {
        shell.run(Command::ViewArchive, "test", cx);
        shell.show_spaces(cx);
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert_eq!(shell.section, Section::Downloads));

    shell.update(cx, |shell, cx| {
        shell.run(Command::ViewMedia, "test", cx);
        let saved = Captured {
            path: "/tmp/capture.png".into(),
            name: "capture.png".into(),
            saved: 1.,
        };
        shell.set_captures(vec![saved], cx);
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.section, Section::Media);
        assert!(
            shell.captures.is_empty(),
            "the regular windows' captures reached a private window"
        );
    });
    host.take();
}

/// Clear in a private window's Downloads clears its session's, and a regular window's the
/// profiles' own.
#[gpui::test]
fn clear_clears_the_windows_own_session(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(true, cx);
    host.take();
    shell.update(cx, |shell, cx| {
        shell
            .browser
            .clone()
            .clear_downloads(shell.private_profile(), cx)
    });
    assert_eq!(host.take(), ["clear downloads Some(1)"]);
}
