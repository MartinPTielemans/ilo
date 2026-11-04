//! Arc's bar commands that call what the menus do: Mute All Tabs and Unmute All Tabs, Enter and
//! Exit Full Screen, Exit (View › Exit) and New Profile, each listed only while it would do
//! something.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::commands::{ShellHandle, bar_commands};
use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};

/// A window showing one Space with `today`'s tabs, each with a page, the bar's key window.
fn window(cx: &mut TestAppContext, today: &[u64]) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_profiles(vec![(0, "Default".into())], cx);
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let (pinned, folders) = (Vec::new(), Folders::default());
        shell.set_tabs(1, pinned, today.to_vec(), Vec::new(), folders, cx);
        for &tab in today {
            let url = format!("https://{tab}.test/").into();
            shell.set_page(
                tab,
                Page {
                    url,
                    ..Page::default()
                },
                cx,
            );
        }
        if let Some(&tab) = today.first() {
            shell.set_selected(tab, cx);
        }
    });
    cx.update(|cx| cx.set_global(ShellHandle(shell.downgrade())));
    (shell, host)
}

/// What the bar lists for `typed` of `commands`, by name, with its key.
fn listed(cx: &mut TestAppContext, typed: &str, commands: &[Command]) -> Vec<String> {
    let found = cx.update(|cx| bar_commands(typed, cx));
    (found.iter())
        .filter(|row| commands.contains(&row.command))
        .map(|row| match &row.shortcut {
            Some(key) => format!("{} {key}", row.name),
            None => row.name.to_string(),
        })
        .collect()
}

/// Mute All Tabs mutes each tab not muted, and Unmute All Tabs is listed while any tab is muted,
/// the way back.
#[gpui::test]
fn mute_all_tabs_mutes_them_and_unmute_all_is_its_way_back(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, &[1, 2]);
    let all = [Command::MuteAll(true), Command::MuteAll(false)];
    assert_eq!(listed(cx, "all tabs", &all), ["Mute All Tabs"]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::MuteAll(true), "command bar", cx)
    });
    let mut muted = host.muted.take();
    muted.sort();
    assert_eq!(muted, [(1, true), (2, true)]);
    shell.update(cx, |shell, cx| {
        let url = "https://1.test/".into();
        shell.set_page(
            1,
            Page {
                url,
                muted: true,
                ..Page::default()
            },
            cx,
        );
    });
    assert_eq!(
        listed(cx, "all tabs", &all),
        ["Mute All Tabs", "Unmute All Tabs"]
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::MuteAll(false), "command bar", cx)
    });
    assert_eq!(host.muted.take(), [(1, false)]);
}

/// Enter Full Screen, found by "toggle" too, reads Exit Full Screen in full screen, and both do
/// what View's item does.
#[gpui::test]
fn enter_full_screen_reads_exit_in_full_screen(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, &[1]);
    let full = [Command::ToggleFullScreen];
    assert_eq!(listed(cx, "toggle", &full), ["Enter Full Screen"]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::ToggleFullScreen, "command bar", cx)
    });
    assert!(host.take().contains(&"toggle full screen".to_owned()));
    shell.update(cx, |shell, _| shell.full_screen = true);
    assert_eq!(listed(cx, "full screen", &full), ["Exit Full Screen"]);
}

/// Exit is View › Exit's: listed only in full screen, with ⎋'s key, and it takes the window out,
/// a page full screen with it, as Exit Full Screen does.
#[gpui::test]
fn exit_leaves_full_screen_and_shows_escape(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, &[1]);
    assert!(
        listed(cx, "exit", &[Command::Exit]).is_empty(),
        "Exit listed out of full screen"
    );
    shell.update(cx, |shell, _| shell.full_screen = true);
    let escape = crate::shell::keystroke_label("escape");
    assert_eq!(
        listed(cx, "exit", &[Command::Exit]),
        [format!("Exit {escape}")]
    );
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Exit, "command bar", cx));
    assert!(host.take().contains(&"toggle full screen".to_owned()));
}

/// New Profile is listed while the Space shown is empty, as the menus' is, and asks for the new
/// profile's name in ilo's dialog first.
#[gpui::test]
fn new_profile_asks_its_name_in_an_empty_space(cx: &mut TestAppContext) {
    let profile = [Command::ChangeProfile(None)];
    let (_, _) = window(cx, &[1]);
    assert!(
        listed(cx, "new profile", &profile).is_empty(),
        "listed with a tab in the Space"
    );
    let (shell, host) = window(cx, &[]);
    assert_eq!(listed(cx, "new profile", &profile), ["New Profile"]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::ChangeProfile(None), "command bar", cx)
    });
    assert_eq!(
        host.told.take().len(),
        1,
        "New Profile didn't ask for the name"
    );
}
