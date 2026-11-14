//! The Help menu's pages and its report of a problem, on a host that records what the shell asks
//! of it: each opens in a tab with no page needed, and the report and the build's details carry
//! the version and system.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::build_info::{self, BuildInfo};
use crate::shell::{Command, Folders, Shell, recorder::Recorder};

/// A window with one Space and no tab shown, as a new window is.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    (shell, host)
}

/// Help Center, What's New and Privacy Policy open the site's pages, from a window with no page
/// too.
#[gpui::test]
fn help_center_whats_new_and_privacy_open_the_sites_pages(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::HelpCenter, "menu", cx);
        shell.run(Command::WhatsNew, "command bar", cx);
        shell.run(Command::PrivacyPolicy, "menu", cx);
    });
    let pages = host.opened_pages.take();
    let site = [
        build_info::HELP_CENTER,
        build_info::WHATS_NEW,
        build_info::PRIVACY_POLICY,
    ];
    assert_eq!(pages, site);
    assert!(pages.iter().all(|page| page.starts_with("https://")));
}

/// Report a Problem… shows the sheet first and opens nothing; Continue opens a new issue on the
/// bug report form with the build's details filled in and what the user typed as its title, and
/// sends nothing itself. Cancel opens nothing. Copy Build Info copies the same details and says
/// so.
#[gpui::test]
fn report_a_problem_fills_the_issue_with_the_builds_details(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    let report = |cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| {
            shell.run(Command::ReportProblem, "menu", cx)
        })
    };
    report(cx);
    assert_eq!(host.told.take(), ["Report a Problem: Continue / Cancel / "]);
    assert!(host.opened_pages.take().is_empty());
    let here = cx.update(|cx| BuildInfo::here(cx));
    let cont = host.told_action.take().expect("the sheet's Continue");
    cx.update(|cx| cont(vec!["  Tabs vanish after sleep ".into()], cx));
    assert_eq!(
        host.opened_pages.take(),
        [here.issue_url(build_info::NEW_ISSUE, "Tabs vanish after sleep")]
    );

    // Another dialog showing, it opens the issue at once, without a title.
    host.dialog_busy.set(true);
    report(cx);
    assert_eq!(
        host.opened_pages.take(),
        [here.issue_url(build_info::NEW_ISSUE, "")]
    );
    host.dialog_busy.set(false);

    shell.update(cx, |shell, cx| {
        shell.run(Command::CopyBuildInfo, "menu", cx)
    });
    assert_eq!(host.copied.take(), [here.text()]);
    assert_eq!(host.toasts.take(), ["Copied Build Info"]);
}

/// Check for Updates… is in the command bar, where Linux has no menu bar to show it, only in a
/// build that updates itself, and asks the browser's updater.
#[gpui::test]
fn check_for_updates_is_in_the_bar_of_a_build_that_updates(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    let offered = |cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, _| shell.offers(Command::CheckForUpdates))
    };
    assert!(!offered(cx), "a dev build has nothing to check");
    host.updates.set(true);
    assert!(offered(cx));
    let found = crate::shell::find_commands("updat", &Default::default(), |name, _| Some(name));
    let row = found
        .iter()
        .find(|row| row.command == Command::CheckForUpdates);
    assert_eq!(row.map(|row| row.name.as_ref()), Some("Check for Updates…"));
    shell.update(cx, |shell, cx| {
        shell.run(Command::CheckForUpdates, "command bar", cx)
    });
    assert_eq!(host.checked_for_updates.get(), 1);
}

/// Once an update is ready, the command bar lists Check for Updates… as Restart to Update, and
/// lists it once, as it does a toggle's row; it's Check for Updates… again for a dev build.
#[gpui::test]
fn check_for_updates_reads_restart_to_update_once_one_is_ready(cx: &mut TestAppContext) {
    use crate::shell::commands::{ShellHandle, bar_commands};
    let (shell, host) = window(cx);
    host.updates.set(true);
    cx.update(|cx| cx.set_global(ShellHandle(shell.downgrade())));
    let names = |cx: &mut TestAppContext| {
        let found = cx.update(|cx| bar_commands("updat", cx));
        let rows = found
            .iter()
            .filter(|row| row.command == Command::CheckForUpdates);
        rows.map(|row| row.name.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(names(cx), ["Check for Updates…"]);
    struct Updater;
    impl crate::updates::Host for Updater {
        fn check(&self, _: &mut gpui::App) {}
        fn show(&self, _: &mut gpui::App) {}
        fn restart(&self, _: &mut gpui::App) {}
        fn follow(&self, _: crate::updates::Track, _: &mut gpui::App) {}
    }
    cx.update(|cx| {
        crate::updates::start(crate::updates::Updater::Own, std::rc::Rc::new(Updater), cx);
        crate::updates::changed(crate::updates::Stage::Ready(Some("0.1.1".into())), cx);
    });
    assert_eq!(names(cx), ["Restart to Update"]);
}
