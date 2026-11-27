//! A page's dialog goes over its tab's card while the tab shows, and its answer reaches the core
//! without what was typed in any log, on a host that records what the shell asks of the core.

use std::rc::Rc;

use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};

use super::{DialogKind, PageDialog, PageReply};
use crate::shell::{Folders, Page, Shell, Split, recorder::Recorder};

/// A window on one Space of tabs 10 and 20, showing 10, drawn.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

fn prompt(id: u64) -> PageDialog {
    PageDialog {
        id,
        kind: DialogKind::Prompt,
        title: "example.org says".into(),
        message: "Your name?".into(),
        text: "".into(),
        accept: "OK".into(),
        cancel: "Cancel".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    }
}

/// What the shell asked about page dialogs since the last time.
fn dialogs(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call.contains("dialog"))
        .collect()
}

fn ask(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, dialog: Option<PageDialog>) {
    shell.update(cx, |shell, cx| shell.set_page_dialog(tab, dialog, cx));
    cx.run_until_parked();
}

fn select(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn the_dialog_follows_its_tab(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // Another tab's waits while its tab is hidden.
    ask(&shell, cx, 20, Some(prompt(2)));
    assert!(dialogs(&host).is_empty());
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
    select(&shell, cx, 10);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // The page took it back, such as when it went to another page.
    ask(&shell, cx, 10, None);
    assert_eq!(dialogs(&host), ["page dialog none"]);
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
}

#[gpui::test]
fn a_window_coming_to_the_front_asks_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    dialogs(&host);
    shell.update(cx, |shell, cx| shell.became_key(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn the_answer_reaches_the_core_once(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    dialogs(&host);
    let reply = || PageReply {
        accept: true,
        text: "Ada".into(),
        password: "".into(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply(), cx));
    cx.run_until_parked();
    // The dialog has left already, so there's nothing to take back.
    assert_eq!(dialogs(&host), ["answer dialog 10 1 accept"]);
    assert_eq!(host.replies.borrow()[0].text, "Ada");
    // An answer to a dialog no longer asked, such as one the page took back, goes nowhere.
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply(), cx));
    cx.run_until_parked();
    assert!(dialogs(&host).is_empty());
}

#[gpui::test]
fn the_password_never_reaches_a_log(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    let reply = PageReply {
        accept: true,
        text: "ada".into(),
        password: "hunter2".into(),
        checked: true,
    };
    assert_eq!(format!("{reply:?}"), "accept, checked");
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply, cx));
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        calls
            .iter()
            .all(|call| !call.contains("hunter2") && !call.contains("ada")),
        "{calls:?}"
    );
    // The core still gets it.
    assert_eq!(host.replies.borrow()[0].password, "hunter2");
}

#[gpui::test]
fn a_tab_in_safaris_engine_keeps_chromiums_dialog_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    let engine = |shell: &Entity<Shell>, cx: &mut VisualTestContext, system_engine: bool| {
        shell.update(cx, |shell, cx| {
            let page = Page {
                system_engine,
                ..shell.page(10)
            };
            shell.set_page(10, page, cx);
        });
        cx.run_until_parked();
    };
    engine(&shell, cx, true);
    assert_eq!(dialogs(&host), ["page dialog none"]);
    engine(&shell, cx, false);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_window_coming_to_the_front_takes_back_anothers_dialog(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    // Another window's page asked, and that window closed with its tab before it drew again, so
    // the dialog over it stayed and took every shortcut after from this one (#3108).
    shell.update(cx, |shell, cx| shell.became_key(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog none"]);
}

#[gpui::test]
fn in_a_split_the_dialog_covers_its_own_pane(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 20],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    let (laid, over) = shell.read_with(cx, |shell, _| {
        let over = shell
            .page_dialog_shown
            .flatten()
            .expect("no dialog shown")
            .2;
        (shell.page_laid.expect("no page laid out"), over)
    });
    // The left pane alone, so the page beside, on the right, stays usable (#3231).
    assert_eq!(over.0.left(), laid.0.left());
    assert_eq!(
        (over.0.top(), over.0.bottom()),
        (laid.0.top(), laid.0.bottom())
    );
    assert!(over.0.right() < laid.0.center().x, "{over:?} in {laid:?}");
    // A click on the page beside focuses its pane, and the dialog stays over its own, at rest,
    // where the page beside keeps the keyboard (#3461).
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 10 1 at rest"]);
    let rests_over = shell.read_with(cx, |shell, _| shell.page_dialog_shown.flatten().unwrap().2);
    assert_eq!(rests_over, over);
    // A click on it at rest focuses its pane, and once the core shows that, it's live again.
    shell.update(cx, |shell, cx| shell.focus_dialog_pane(10, cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"select 10".to_string()));
    select(&shell, cx, 10);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // Live, its pane is focused already.
    shell.update(cx, |shell, cx| shell.focus_dialog_pane(10, cx));
    cx.run_until_parked();
    assert!(host.take().is_empty());
}

#[gpui::test]
fn closing_the_page_beside_keeps_the_resting_dialog_up(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let split = Split {
        tabs: vec![10, 20],
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(20, cx);
    });
    cx.run_until_parked();
    host.take();
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1 at rest"]);
    // ⌘W closes the page beside. The core reports the Space without it before it says the window
    // shows the dialog's tab, and a frame drawn in between shows no tab: the dialog stays.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
    });
    cx.run_until_parked();
    assert_eq!(dialogs(&host), Vec::<String>::new());
    // Shown alone, its tab has it live.
    select(&shell, cx, 10);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn closing_the_tab_shown_takes_back_the_dialog_of_a_tab_that_isnt(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let split = Split {
        tabs: vec![10, 20],
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(20, cx);
    });
    cx.run_until_parked();
    ask(&shell, cx, 10, Some(prompt(1)));
    host.take();
    // The dialog's own tab closes with the tab shown: nothing keeps it.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![30], Vec::new(), Folders::default(), cx);
    });
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog none"]);
}

#[gpui::test]
fn in_a_split_the_focused_panes_dialog_asks_first(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 20],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20, 30],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    // A tab outside the split waits for its tab to show, as anywhere.
    ask(&shell, cx, 30, Some(prompt(3)));
    assert_eq!(dialogs(&host), Vec::<String>::new());
    // The pane beside asks at rest while the one the user is in asks nothing.
    ask(&shell, cx, 20, Some(prompt(2)));
    assert_eq!(dialogs(&host), ["page dialog 20 2 at rest"]);
    // The user's pane asking comes first, live; the other's waits.
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    ask(&shell, cx, 10, None);
    assert_eq!(dialogs(&host), ["page dialog 20 2 at rest"]);
    // Its pane focused by keys, it's live.
    shell.update(cx, |shell, cx| shell.focus_pane(true, cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"select 20".to_string()));
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
}

/// The command bar's open, as ⌘T or ⌘L opened it, and the user types in it.
fn open_bar(host: &Recorder) {
    host.bar_open.set(true);
}

/// The bar closed, as with Esc or a choice, which tells the shell.
fn close_bar(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) {
    host.bar_open.set(false);
    shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    cx.run_until_parked();
}

fn leave(id: u64) -> PageDialog {
    PageDialog {
        kind: DialogKind::Leave,
        ..prompt(id)
    }
}

#[gpui::test]
fn a_dialog_waits_while_the_command_bar_is_open(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    open_bar(&host);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert!(dialogs(&host).is_empty(), "the dialog came over the bar");
    // Another draw while the bar is open, as a page title changing gives, keeps it back.
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(dialogs(&host).is_empty(), "the dialog came over the bar");
    close_bar(&shell, cx, &host);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_dialog_waits_while_a_sidebar_field_has_the_keyboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.rename_tab(10, cx));
    cx.run_until_parked();
    ask(&shell, cx, 10, Some(prompt(1)));
    cx.simulate_input("Ada");
    assert!(dialogs(&host).is_empty(), "the dialog came over the field");
    let typed = shell.read_with(cx, |shell, cx| {
        shell
            .renaming
            .as_ref()
            .map(|renaming| renaming.name.read(cx).text().to_string())
    });
    assert_eq!(typed.as_deref(), Some("Ada"));
    // ↩ keeps the name and lets the keyboard go, and the dialog comes with that draw.
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

/// A dialog that waited for a field shows in a split as it would have at once: at rest over the
/// pane beside, live over the one the user is in (#3461).
#[gpui::test]
fn a_dialog_that_waited_for_the_bar_rests_or_goes_live_by_its_pane(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 20],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            vec![split],
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    host.take();
    open_bar(&host);
    ask(&shell, cx, 20, Some(prompt(2)));
    assert!(dialogs(&host).is_empty(), "the dialog came over the bar");
    close_bar(&shell, cx, &host);
    assert_eq!(dialogs(&host), ["page dialog 20 2 at rest"]);
    ask(&shell, cx, 20, None);
    host.take();
    open_bar(&host);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert!(dialogs(&host).is_empty(), "the dialog came over the bar");
    close_bar(&shell, cx, &host);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_leave_doesnt_wait(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    open_bar(&host);
    ask(&shell, cx, 10, Some(leave(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_dialog_up_stays_up_as_the_bar_opens(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    open_bar(&host);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(dialogs(&host).is_empty(), "the dialog up was taken down");
    // The page asking again is a dialog not up yet, which waits.
    ask(&shell, cx, 10, Some(prompt(2)));
    assert_eq!(dialogs(&host), ["page dialog none"]);
    close_bar(&shell, cx, &host);
    assert_eq!(dialogs(&host), ["page dialog 10 2"]);
}

#[gpui::test]
fn a_dialog_waits_while_find_has_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    host.find_typing.set(true);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert!(dialogs(&host).is_empty(), "the dialog came over find");
    // The user went back to the page, or closed find, which tells the shell.
    host.find_typing.set(false);
    shell.update(cx, |shell, cx| shell.field_let_go(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_field_no_longer_drawn_doesnt_hold_a_dialog(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.rename_tab(10, cx));
    cx.run_until_parked();
    // The field goes from the sidebar with GPUI's focus still on it, kept alive here.
    let field = shell.update(cx, |shell, cx| {
        let field = shell.renaming.take().map(|renaming| renaming.name);
        cx.notify();
        field
    });
    ask(&shell, cx, 10, Some(prompt(1)));
    let focused = cx.update(|window, cx| {
        field
            .as_ref()
            .is_some_and(|field| field.focus_handle(cx).is_focused(window))
    });
    assert!(focused, "the test lost the case it's for");
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

/// The host says the page took the keyboard from a field that stays drawn with GPUI's focus, as
/// the Library's search can: a prompt held for it shows at once. A press back on the same field
/// gives it the keys again, so the next prompt waits until the field lets go. A rename ends as the
/// page takes the keyboard, so the test tells the shell only what the host's word tells it.
#[gpui::test]
fn the_page_taking_the_keyboard_lets_a_dialog_go_up(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.rename_tab(10, cx));
    cx.run_until_parked();
    ask(&shell, cx, 10, Some(prompt(1)));
    assert!(dialogs(&host).is_empty(), "the prompt came over the field");

    shell.update_in(cx, |shell, window, cx| shell.page_took_keyboard(window, cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    let reply = PageReply {
        accept: true,
        text: "".into(),
        password: "".into(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply, cx));
    cx.run_until_parked();
    host.take();

    let field = shell.read_with(cx, |shell, _| {
        shell
            .renaming
            .as_ref()
            .map(|renaming| renaming.name.clone())
    });
    let field = field.expect("the field went");
    let at = field.read_with(cx, |field, _| field.drawn_bounds().expect("drawn").center());
    cx.simulate_click(at, gpui::Modifiers::none());
    let focused = cx.update(|window, cx| field.focus_handle(cx).is_focused(window));
    assert!(focused, "the press moved the keyboard off the field");
    ask(&shell, cx, 10, Some(prompt(2)));
    assert!(dialogs(&host).is_empty(), "the prompt came over the field");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 2"]);
}

/// Where the core says who has the keys, as on Linux, the shell takes its word.
#[gpui::test]
fn the_cores_word_on_the_keyboard_comes_first(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| shell.rename_tab(10, cx));
    cx.run_until_parked();
    host.page_keys.set(Some(true));
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    host.page_keys.set(Some(false));
    ask(&shell, cx, 10, Some(prompt(2)));
    assert_eq!(dialogs(&host), ["page dialog none"]);
}

/// The UI taking the keyboard back by a key, with GPUI's focus on the field it had when the page
/// took it, holds the next prompt as a press there would.
#[gpui::test]
fn the_ui_taking_the_keyboard_back_by_key_holds_a_dialog(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    use crate::shell::Command;
    let library =
        |shell: &Entity<Shell>, command: Command, from: &str, cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| {
                shell.run(command, from, cx);
                shell.library_motion = None;
                shell.sidebar_fade = None;
            });
            cx.run_until_parked();
        };
    library(&shell, Command::ViewLibrary, "ui", cx);
    // The Library's search has the keyboard, drawn.
    let search = cx.update(|window, cx| window.focused(cx).expect("nothing has the keyboard"));
    assert!(cx.update(|window, cx| crate::text_input::typing_in(window, cx)));
    let on_search = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.focused(cx).as_ref() == Some(&search))
    };

    shell.update_in(cx, |shell, window, cx| shell.page_took_keyboard(window, cx));
    cx.run_until_parked();
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(
        dialogs(&host),
        ["page dialog 10 1"],
        "the page's prompt waited"
    );
    let reply = PageReply {
        accept: true,
        text: "".into(),
        password: "".into(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply, cx));
    cx.run_until_parked();
    host.take();

    // Tab into the open Library gives its search, which GPUI's focus never left, the keyboard
    // back, with no press.
    shell.update(cx, |shell, cx| assert!(shell.tab_into_library(cx)));
    cx.run_until_parked();
    assert!(on_search(cx), "the key moved the focus off the search");
    assert!(
        host.take().contains(&"focus ui".to_owned()),
        "the UI didn't take the keyboard"
    );
    ask(&shell, cx, 10, Some(prompt(2)));
    assert!(dialogs(&host).is_empty(), "the prompt came over the search");
    // The Library's key closes it, and its search lets go.
    library(&shell, Command::ViewLibrary, "ui", cx);
    assert!(!shell.read_with(cx, |shell, _| shell.library_open));
    assert_eq!(dialogs(&host), ["page dialog 10 2"]);
}

/// Windows' word that the UI lost the keyboard as another app comes to the front leaves where the
/// keys were (#3758): back in the app, a prompt goes up at once if the page had them, and waits if
/// the Library's search still drawn with GPUI's focus had them.
#[gpui::test]
fn the_app_going_to_the_background_leaves_the_keyboard_where_it_was(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, cx, host) = window(cx);
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        shell.run(crate::shell::Command::ViewLibrary, "ui", cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    assert!(cx.update(|window, cx| crate::text_input::typing_in(window, cx)));
    let background = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let opened = crate::shell::fields_opened();
        shell.update_in(cx, |shell, window, cx| {
            shell.keyboard_left(opened, false, window, cx)
        });
        cx.run_until_parked();
    };

    // The search has the keys as another app comes to the front.
    background(&shell, cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert!(dialogs(&host).is_empty(), "the prompt came over the search");
    ask(&shell, cx, 10, None);
    host.take();

    // The user went to the page, then to another app.
    shell.update_in(cx, |shell, window, cx| shell.page_took_keyboard(window, cx));
    cx.run_until_parked();
    background(&shell, cx);
    ask(&shell, cx, 10, Some(prompt(2)));
    assert_eq!(
        dialogs(&host),
        ["page dialog 10 2"],
        "the page's prompt waited for the search"
    );
}

/// A tab that waits on its user, for a permission, an HTTP sign-in or one of its page's dialogs,
/// has the needs-you mark on its row until it's answered, shown or not (#4410).
#[gpui::test]
fn a_tab_that_waits_has_the_mark_until_answered(cx: &mut TestAppContext) {
    use crate::shell::Question;
    let (shell, cx, _) = window(cx);
    let marked = |shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64| {
        shell.read_with(cx, |shell, _| shell.waiting_mark(tab).is_some())
    };
    assert!(!marked(&shell, cx, 20));
    let camera = Question {
        site: "example.org".into(),
        permissions: vec!["Use your camera".into()],
    };
    shell.update(cx, |shell, cx| shell.set_question(20, Some(camera), cx));
    assert!(marked(&shell, cx, 20) && !marked(&shell, cx, 10));
    shell.update(cx, |shell, cx| shell.set_question(20, None, cx));
    assert!(!marked(&shell, cx, 20));
    for kind in [DialogKind::Alert, DialogKind::SignIn] {
        ask(&shell, cx, 20, Some(PageDialog { kind, ..prompt(5) }));
        assert!(marked(&shell, cx, 20), "{kind:?}");
        let reply = PageReply {
            accept: true,
            text: String::new(),
            password: String::new(),
            checked: false,
        };
        shell.update(cx, |shell, cx| shell.answer_page_dialog(20, 5, reply, cx));
        assert!(!marked(&shell, cx, 20), "{kind:?}");
    }
}
