//! A task's Space and the page area showing it, on a host that records what the shell asks of the
//! core. The core's answers are played back as it posts them: the Space shown, then the tab.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyView, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, TestAppContext, VisualTestContext, Window,
    div, point, px, size,
};

use super::{Zoom, pushed, watched};
use crate::agents::Status;
use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};
use crate::tokens::shell::conversation::ilo::ARM;

/// The user's Space 1 with tabs 10 and 11, shown with 10, Claude Code's own Space 2 with tab 20,
/// Space 3 of the task task-1, run by Claude Code, with tabs 30 and 31, and the user's empty
/// Space 4.
fn shell_with_task(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    host.space_agents
        .borrow_mut()
        .insert(2, "claude-code".into());
    host.space_agents.borrow_mut().insert(3, "task-1".into());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
                (
                    3,
                    "Find a flight".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
                (
                    4,
                    "Errands".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
            ],
            cx,
        );
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_tabs(
            3,
            Vec::new(),
            vec![30, 31],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    host.shown.borrow_mut().clear();
    (shell, host, cx)
}

struct Conversation;

impl Render for Conversation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn conversation(cx: &mut VisualTestContext) -> AnyView {
    cx.update(|_, cx| cx.new(|_| Conversation).into())
}

fn still(cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.set_reduce_motion(true));
}

/// Draws the shell again, as the app's next frame does.
fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// The card's frame, and where the page was laid out last.
fn frames(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Bounds<Pixels>, Bounds<Pixels>) {
    shell.read_with(cx, |shell, _| {
        let (card, _) = shell.page_frame(shell.viewport);
        (card, shell.page_laid().expect("laid out").0)
    })
}

fn away(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Pixels {
    shell.read_with(cx, |shell, _| {
        shell.viewport.width - shell.page_frame(shell.viewport).0.left()
    })
}

/// The core shows the task's Space with `tab`, as it does when the shell selects one of its tabs.
fn core_shows_task(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| {
        shell.set_space(3, cx);
        shell.set_selected(tab, cx);
    });
}

/// And the user's Space again, with their tab.
fn core_shows_users(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        let tab = shell.selected();
        shell.set_space(1, cx);
        shell.set_selected(tab, cx);
    });
}

/// The watched page's zoom.
fn zoom(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Zoom {
    shell.read_with(cx, |shell, _| {
        shell
            .task_area
            .as_ref()
            .unwrap()
            .page
            .as_ref()
            .unwrap()
            .zoom
    })
}

fn set_zoom(shell: &Entity<Shell>, cx: &mut VisualTestContext, zoom: Zoom) {
    shell.update(cx, |shell, _| {
        shell
            .task_area
            .as_mut()
            .unwrap()
            .page
            .as_mut()
            .unwrap()
            .zoom = zoom;
    });
}

/// A conversation open, and the task's tab 30 watched, at rest in place, with the core showing it.
fn watching(shell: &Entity<Shell>, host: &Recorder, cx: &mut VisualTestContext) {
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    }));
    core_shows_task(shell, cx, 30);
    draw(shell, cx);
    host.take();
    host.shown.borrow_mut().clear();
}

/// A page ilo opens by itself, as at a handoff, never takes the keyboard, even one the user is
/// handed; and for ARM after it opens it stays behind its picture, so a press meant for what
/// stood there doesn't reach it. Then it's in place, where the user's first click gives it the
/// keyboard, as on a page they opened.
#[gpui::test]
fn a_page_ilo_opens_takes_no_keyboard_and_no_click_at_first(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(30, "Sign in".into())],
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    draw(&shell, cx);
    host.take();
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, true, cx)
    }));
    core_shows_task(&shell, cx, 30);
    draw(&shell, cx);
    let calls = host.take();
    assert!(
        !calls
            .iter()
            .any(|call| call == "focus page" || call == "focus ui"),
        "the page ilo opened moved the keyboard: {calls:?}"
    );
    let live = |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.task_page_live());
    assert_eq!(live(cx), None, "the page took a click as it opened");
    shell.update(cx, |shell, cx| shell.press_task_page(cx));
    draw(&shell, cx);
    assert_eq!(live(cx), None, "a press inside ARM reached the page");

    cx.executor()
        .advance_clock(std::time::Duration::from_secs_f32(ARM + 0.1));
    draw(&shell, cx);
    assert_eq!(live(cx), Some(30), "the page isn't in place after ARM");
    assert!(
        !host.take().iter().any(|call| call == "focus page"),
        "the page took the keyboard by itself once in place"
    );
}

/// A task's Space, and an outside agent's, whose sessions are cards as tasks are (#1080), is in
/// none of the window's lists of Spaces: not the Spaces menu, ⌃1 to ⌃9, Move to or Search Tabs.
/// The task's tabs are kept for its conversation.
#[gpui::test]
fn a_tasks_space_is_never_listed(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    assert_eq!(host.menu_bar.borrow().spaces, ["Space 1", "Errands"]);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(2), "ui", cx));
    assert!(host.shown.borrow().is_empty(), "no third Space for ⌃3");
    shell.update(cx, |shell, cx| {
        shell.tab_menu(10, point(px(100.), px(200.)), cx)
    });
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    let titles: Vec<SharedString> = menu.into_iter().map(|(title, _)| title).collect();
    assert!(titles.contains(&"Errands".into()), "{titles:?}");
    assert!(!titles.contains(&"Research".into()), "{titles:?}");
    assert!(!titles.contains(&"Find a flight".into()), "{titles:?}");
    shell.update(cx, |shell, cx| {
        for tab in [11, 20, 30, 31] {
            let page = Page {
                url: format!("https://{tab}.test/").into(),
                ..Page::default()
            };
            shell.set_page(tab, page, cx);
        }
    });
    let searched = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
    let searched: Vec<u64> = searched.iter().map(|tab| tab.tab).collect();
    assert_eq!(
        searched,
        [11],
        "the agents' 20, 30 and 31 aren't the user's to search"
    );
    let bar = shell.read_with(cx, |shell, cx| shell.bar_tabs(cx));
    assert_eq!(bar.iter().map(|tab| tab.tab).collect::<Vec<_>>(), [11]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_tabs(3)), [30, 31]);
}

/// The core showing a task's Space without its page watched, as a launch that brings it back
/// does, gives way to the user's Space, and the window's Space and tab stay the user's.
#[gpui::test]
fn the_core_showing_a_tasks_space_gives_way_to_the_users(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    core_shows_task(&shell, cx, 30);
    assert_eq!(*host.shown.borrow(), [1]);
    assert_eq!(
        shell.read_with(cx, |shell, _| (shell.space(), shell.selected())),
        (1, 10)
    );
}

/// The conversation pushes the Space's page out of the window at its size, as the Library pushes
/// the card, and gives it back on the way out, which gives the page the keyboard again.
#[gpui::test]
fn the_conversation_pushes_the_page_out_and_gives_it_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let (card, laid) = frames(&shell, cx);
    assert_eq!(laid, card);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    let (card, laid) = frames(&shell, cx);
    assert_eq!(laid, pushed(card, away(&shell, cx)));
    assert_eq!(laid.size, card.size, "only moved");
    assert_eq!(host.take(), ["focus ui"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, card);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(host.take(), ["focus page"]);
    assert!(
        host.shown.borrow().is_empty(),
        "the core showed the user's Space throughout"
    );
}

/// Even with motion, the conversation shows in the page's place at once, as a tab switch does, the
/// page going out with it, and the page comes back at once on the way out.
#[gpui::test]
fn the_conversation_shows_at_once(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with_task(cx);
    let (card, _) = frames(&shell, cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    shell.update(cx, |shell, cx| shell.close_task(cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, card);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// A task's page watched: the core shows the task's Space with it, laid out in place above the
/// strip's room, while the window's Space, tab and selection stay the user's. Back gives the
/// conversation, Escape with the UI's keyboard too, and the second Escape the user's page, and
/// neither stops the agent.
#[gpui::test]
fn a_tasks_page_shows_in_place_and_steps_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let (card, laid) = frames(&shell, cx);
    assert_eq!(laid, watched(card));
    assert_eq!(
        card.size.height - laid.size.height,
        px(crate::tokens::shell::task_area::PAGE_ROOM)
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| (shell.space(), shell.selected())),
        (1, 10)
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );

    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    assert_eq!(
        *host.shown.borrow(),
        [1],
        "the core shows the user's Space again"
    );
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );

    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, card);
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("stop")),
        "{calls:?}"
    );
}

/// A dialog asked by tab `tab`'s page, as the core reports it.
fn page_dialog(id: u64, kind: crate::shell::DialogKind) -> crate::shell::PageDialog {
    crate::shell::PageDialog {
        id,
        kind,
        title: "Sign in".into(),
        message: "http://auth.example\nYour connection to this site is not private".into(),
        text: "".into(),
        accept: "Sign in".into(),
        cancel: "Cancel".into(),
        check: "".into(),
        username: "Username".into(),
        password: "Password".into(),
    }
}

/// What the shell asked about page dialogs since the last time.
fn dialogs(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|call| call.contains("dialog"))
        .collect()
}

/// A task's page watched shows its sign-in over it, which only the user may answer (#3662), and
/// takes it back as the area goes back to the conversation. Its other dialogs wait, as does the
/// user's own page's while it's out of the window.
#[gpui::test]
fn a_tasks_page_watched_shows_its_sign_in(cx: &mut TestAppContext) {
    use crate::shell::DialogKind;
    let (shell, host, cx) = shell_with_task(cx);
    let ask = |cx: &mut VisualTestContext, tab, dialog| {
        shell.update(cx, |shell, cx| shell.set_page_dialog(tab, dialog, cx));
        draw(&shell, cx);
    };
    ask(cx, 30, Some(page_dialog(1, DialogKind::SignIn)));
    ask(cx, 10, Some(page_dialog(2, DialogKind::Confirm)));
    assert_eq!(dialogs(&host), ["page dialog 10 2"]);
    ask(cx, 10, None);
    host.take();

    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    assert!(dialogs(&host).iter().all(|call| call == "page dialog none"));
    watching(&shell, &host, cx);
    let shown = shell.read_with(cx, |shell, _| shell.page_dialog_shown);
    assert_eq!(
        shown.flatten().map(|(tab, id, _, _)| (tab, id)),
        Some((30, 1))
    );

    ask(cx, 30, Some(page_dialog(3, DialogKind::Confirm)));
    assert_eq!(dialogs(&host), ["page dialog none"]);
    ask(cx, 30, Some(page_dialog(4, DialogKind::SignIn)));
    assert_eq!(dialogs(&host), ["page dialog 30 4"]);
    ask(cx, 10, Some(page_dialog(5, DialogKind::SignIn)));
    assert!(
        dialogs(&host).is_empty(),
        "the user's page asked over the task's"
    );

    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    assert_eq!(dialogs(&host), ["page dialog none"]);
}

/// A task's page watched asks for its sign-in while the user types in the command bar: the dialog
/// waits, and goes up as the bar closes.
#[gpui::test]
fn a_tasks_sign_in_waits_while_the_user_types(cx: &mut TestAppContext) {
    use crate::shell::DialogKind;
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    host.bar_open.set(true);
    shell.update(cx, |shell, cx| {
        shell.set_page_dialog(30, Some(page_dialog(1, DialogKind::SignIn)), cx)
    });
    draw(&shell, cx);
    draw(&shell, cx);
    assert!(dialogs(&host).is_empty(), "the sign-in came over the bar");
    host.bar_open.set(false);
    shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 30 1"]);
}

/// With motion, the page zooms out of its thumbnail, laid out out of the window till it's in
/// place, and back into it from a picture of it as it is.
#[gpui::test]
fn a_tasks_page_zooms_out_of_its_thumbnail_and_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    cx.update(|_, cx| cx.set_reduce_motion(false));
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    host.take();
    // The zooms run on the real clock, so the test sets each one's start: a minute ahead holds it
    // where it starts however slow the runner, and a second back has it over.
    let begun = Instant::now() + Duration::from_secs(60);
    let over = Instant::now() - Duration::from_secs(1);
    shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    });
    assert_eq!(host.take(), ["select 30", "focus ui"]);
    assert!(matches!(zoom(&shell, cx), Zoom::Out(_)), "it zooms out");
    set_zoom(&shell, cx, Zoom::Out(begun));
    core_shows_task(&shell, cx, 30);
    draw(&shell, cx);
    let (card, laid) = frames(&shell, cx);
    assert_eq!(
        laid,
        pushed(watched(card), away(&shell, cx)),
        "out of the window as it zooms"
    );
    set_zoom(&shell, cx, Zoom::Out(over));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, watched(card), "in place");

    shell.update(cx, |shell, cx| shell.task_page_back(cx));
    let calls = host.take();
    assert_eq!(calls, ["picture 30", "focus ui"]);
    let Zoom::Asked(request) = zoom(&shell, cx) else {
        panic!("{:?}", zoom(&shell, cx));
    };
    assert!(
        host.shown.borrow().is_empty(),
        "the page stays till its picture comes"
    );
    shell.update(cx, |shell, cx| shell.page_pictured(request, None, cx));
    assert_eq!(*host.shown.borrow(), [1]);
    assert!(matches!(zoom(&shell, cx), Zoom::In(_)), "it zooms back in");
    set_zoom(&shell, cx, Zoom::In(begun));
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_page_drawn()),
        Some((3, 30)),
        "drawn as it zooms in"
    );
    set_zoom(&shell, cx, Zoom::In(over));
    draw(&shell, cx);
    assert!(shell.read_with(cx, |shell, _| {
        shell.task_area.as_ref().unwrap().page.is_none()
    }));
}

/// A conversation whose pictures fill it, which counts the clicks on them, as a picture's watches
/// its page.
struct Pictures(Rc<std::cell::Cell<u32>>);

impl Render for Pictures {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.0.clone();
        div()
            .id("pictures")
            .size_full()
            .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
    }
}

/// A click on another picture while the page watched goes back into its thumbnail reaches the
/// picture, which watches its own page, as one does once the page is back (#3496). The page going
/// back is drawn over the conversation, and its press there is for a page coming out alone.
#[gpui::test]
fn a_click_on_the_conversation_while_the_page_goes_back_reaches_it(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shell_with_task(cx);
    let clicks = Rc::new(std::cell::Cell::new(0));
    let pictures = clicks.clone();
    let view: AnyView = cx.update(|_, cx| cx.new(|_| Pictures(pictures)).into());
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    });
    core_shows_task(&shell, cx, 30);
    let over = Instant::now() - Duration::from_secs(1);
    set_zoom(&shell, cx, Zoom::Out(over));
    draw(&shell, cx);
    let (card, _) = frames(&shell, cx);
    // Held where the zoom back starts, the page still fills the area.
    set_zoom(
        &shell,
        cx,
        Zoom::In(Instant::now() + Duration::from_secs(60)),
    );
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    cx.simulate_click(card.center(), gpui::Modifiers::none());
    assert_eq!(clicks.get(), 1, "the click on the conversation was lost");
}

/// Selecting any of the user's tabs, the one selected too, or another Space leaves the area and
/// shows it. The window's own commands, such as ⌘S, and Focus Agent Question leave the area as
/// it is.
#[gpui::test]
fn selecting_a_tab_or_a_space_leaves_the_area(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
    assert!(shell.read_with(cx, |shell, _| shell.task_shown().is_some()));
    // Focus Agent Question too, from the page or the UI: the card shows over the area (#3869).
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "page", cx);
        shell.run(Command::FocusAgentQuestion, "ui", cx);
    });
    assert!(shell.read_with(cx, |shell, _| shell.task_shown().is_some()));
    host.take();
    shell.update(cx, |shell, cx| shell.select(10, cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(
        host.take(),
        ["select 10"],
        "shown again, as the core shows the task's tab"
    );
    core_shows_users(&shell, cx);

    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(1), "ui", cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(
        *host.shown.borrow(),
        [1, 4],
        "the user's Space back, then the second"
    );

    let view = conversation(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(1, cx);
        shell.open_task(3, view, cx);
        shell.select(11, cx);
    });
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    core_shows_users(&shell, cx);
}

/// ⌘Q with a task's view shown asks first, and Cancel leaves the window as it was, the task's
/// page watched still showing (#4202). Once the quit is decided it's as the window's close: the
/// page watched goes back to the conversation, so the core shows the user's Space again, which
/// the session then keeps for the window.
#[gpui::test]
fn quit_leaves_the_tasks_view_until_the_quit_is_decided(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let shown = shell.read_with(cx, |shell, _| shell.task_shown());
    assert!(shown.is_some());
    shell.update(cx, |shell, cx| shell.run(Command::Quit, "ui", cx));
    assert_eq!(*host.app_asks.borrow(), ["quit"]);
    // Cancel is the host never saying the quit goes ahead.
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        shown,
        "the view closed before the answer"
    );
    assert!(
        host.shown.borrow().is_empty(),
        "the core was shown another Space before the answer"
    );

    let going = host.quit_going.borrow_mut().take();
    let going = going.expect("Quit gave the host nothing to run once it's decided");
    cx.update(|_, cx| going(cx));
    assert_eq!(*host.shown.borrow(), [1]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
}

/// ⇧⌘W with a task's view shown leaves its conversation standing: the close is the core's, which
/// asks the window's pages first, and their Cancel leaves the window as it was (#4202). A page of
/// the task's watched goes back to the conversation before the close, so the core shows the
/// user's Space again, which the session keeps for the window.
#[gpui::test]
fn close_window_leaves_the_tasks_conversation(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::CloseWindow, "ui", cx));
    assert!(host.take().contains(&"close window".to_string()));
    // Cancel is the core never saying the window closed.
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None)),
        "the view closed before the page's answer"
    );
    assert!(
        host.shown.borrow().is_empty(),
        "the core was shown a Space it already showed"
    );

    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::CloseWindow, "ui", cx));
    assert!(host.take().contains(&"close window".to_string()));
    assert_eq!(
        *host.shown.borrow(),
        [1],
        "the window closed with the core showing the task's Space"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None)),
        "the conversation went with the page watched"
    );
}

/// The user's page 10 asks whether to leave it, as the core reports it while the window closes.
fn page_asks_to_stay(shell: &Entity<Shell>, cx: &mut VisualTestContext, id: u64) {
    use crate::shell::DialogKind;
    shell.update(cx, |shell, cx| {
        shell.set_page_dialog(10, Some(page_dialog(id, DialogKind::Leave)), cx)
    });
    draw(shell, cx);
}

/// The user's answer to that question.
fn answer(shell: &Entity<Shell>, cx: &mut VisualTestContext, id: u64, leave: bool) {
    let reply = crate::shell::PageReply {
        accept: leave,
        text: String::new(),
        password: String::new(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, id, reply, cx));
    draw(shell, cx);
}

/// Closes the window one of the two ways: ⇧⌘W, or ⌘Q with its own question answered.
fn close(shell: &Entity<Shell>, host: &Recorder, cx: &mut VisualTestContext, quit: bool) {
    if !quit {
        return shell.update(cx, |shell, cx| shell.run(Command::CloseWindow, "ui", cx));
    }
    shell.update(cx, |shell, cx| shell.run(Command::Quit, "ui", cx));
    let going = host.quit_going.borrow_mut().take();
    let going = going.expect("Quit gave the host nothing to run once it's decided");
    cx.update(|_, cx| going(cx));
}

/// ⇧⌘W, or a quit decided, with a task's conversation standing and a page of the user's that
/// asks before it's left: the page and its question show, as no page's dialog is drawn over a
/// conversation, and Cancel brings the conversation back with the keyboard, the page out of the
/// window again (#4202). The core never says a close was refused: the page's answer is that word.
#[gpui::test]
fn a_page_that_refuses_the_close_brings_the_conversation_back(cx: &mut TestAppContext) {
    for quit in [false, true] {
        let (shell, host, cx) = shell_with_task(cx);
        still(cx);
        let view = conversation(cx);
        shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
        draw(&shell, cx);
        let (card, _) = frames(&shell, cx);
        let away = away(&shell, cx);
        close(&shell, &host, cx, quit);
        host.take();

        page_asks_to_stay(&shell, cx, 7);
        assert_eq!(
            dialogs(&host),
            ["page dialog 10 7"],
            "the page's question never showed (quit: {quit})"
        );
        assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
        assert_eq!(
            frames(&shell, cx).1,
            card,
            "the page asks out of the window"
        );

        answer(&shell, cx, 7, false);
        let asked = host.take();
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, None)),
            "Cancel didn't bring the conversation back (quit: {quit})"
        );
        assert_eq!(frames(&shell, cx).1, pushed(card, away));
        let at = |call: &str| asked.iter().position(|each| each == call);
        assert!(at("focus ui").is_some(), "{asked:?}");
        assert!(
            at("focus ui") < at("answer dialog 10 7 cancel"),
            "the page was answered before the view was back: {asked:?}"
        );
        assert!(
            host.shown.borrow().is_empty(),
            "the core was shown a Space it already showed"
        );

        // The close it refused is over: a question the page asks by itself waits as ever.
        page_asks_to_stay(&shell, cx, 8);
        assert!(dialogs(&host).iter().all(|call| call == "page dialog none"));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, None))
        );
    }
}

/// The same from a page of the task's watched: it goes back to the conversation for the close,
/// and Cancel on the user's page's question brings it back in place, watched, with the keyboard
/// where a page watched has it: the page's if the user holds it, else the UI's.
#[gpui::test]
fn a_page_that_refuses_the_close_brings_the_page_watched_back(cx: &mut TestAppContext) {
    for (quit, held) in [(false, false), (true, false), (false, true)] {
        let (shell, host, cx) = shell_with_task(cx);
        watching(&shell, &host, cx);
        let held_tabs: Vec<(u64, SharedString)> = match held {
            true => vec![(30, "".into())],
            false => Vec::new(),
        };
        shell.update(cx, |shell, cx| {
            let status = Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Typed · flights.example".into(),
                held: held_tabs,
            };
            shell.set_agent_status(vec![status], cx);
            shell.agent_acted("task-1".into(), 30, cx);
        });
        draw(&shell, cx);
        host.take();

        close(&shell, &host, cx, quit);
        assert_eq!(*host.shown.borrow(), [1]);
        host.shown.borrow_mut().clear();
        core_shows_users(&shell, cx);
        host.take();
        page_asks_to_stay(&shell, cx, 7);
        assert_eq!(
            dialogs(&host),
            ["page dialog 10 7"],
            "the page's question never showed (quit: {quit})"
        );

        answer(&shell, cx, 7, false);
        let asked = host.take();
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, Some(30))),
            "Cancel didn't bring the page watched back (quit: {quit}, held: {held})"
        );
        assert_eq!(zoom(&shell, cx), Zoom::Rest);
        let at = |call: &str| asked.iter().position(|each| each == call);
        assert!(at("select 30").is_some(), "{asked:?}");
        let keyboard = if held { "focus page" } else { "focus ui" };
        assert!(
            at("select 30") < at(keyboard),
            "the keyboard isn't where the page watched has it (held: {held}): {asked:?}"
        );
        core_shows_task(&shell, cx, 30);
        draw(&shell, cx);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_page_live()),
            Some(30)
        );
    }
}

/// Leave on that question goes on with the close, the window on the user's page, and what the
/// user does next is no answer to the close: the view kept for its Cancel is let go.
#[gpui::test]
fn leaving_the_page_goes_on_with_the_close(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    close(&shell, &host, cx, false);
    page_asks_to_stay(&shell, cx, 7);
    host.take();
    answer(&shell, cx, 7, true);
    assert!(
        host.take()
            .contains(&"answer dialog 10 7 accept".to_string())
    );
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);

    // A second page of theirs refuses: the conversation is back, as the window stays.
    page_asks_to_stay(&shell, cx, 8);
    answer(&shell, cx, 8, false);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );

    // And a command meanwhile lets the view go: a later Cancel is some other question's.
    close(&shell, &host, cx, false);
    page_asks_to_stay(&shell, cx, 9);
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
    answer(&shell, cx, 9, false);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// The same close asked again while its question is up, ⇧⌘W twice or ⌘Q twice, is no other
/// command: the question stays, the view stays kept, and Cancel brings it back as it stood, a
/// page watched with it (#4202).
#[gpui::test]
fn the_same_close_again_keeps_the_view(cx: &mut TestAppContext) {
    for (quit, watched) in [(false, false), (true, false), (false, true), (true, true)] {
        let (shell, host, cx) = shell_with_task(cx);
        if watched {
            watching(&shell, &host, cx);
        } else {
            still(cx);
            let view = conversation(cx);
            shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
            draw(&shell, cx);
        }
        close(&shell, &host, cx, quit);
        if watched {
            core_shows_users(&shell, cx);
        }
        host.take();
        page_asks_to_stay(&shell, cx, 7);
        assert_eq!(dialogs(&host), ["page dialog 10 7"]);

        close(&shell, &host, cx, quit);
        draw(&shell, cx);
        assert!(
            dialogs(&host).iter().all(|call| call != "page dialog none"),
            "the question went at the second press (quit: {quit})"
        );
        assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);

        answer(&shell, cx, 7, false);
        let stood = Some((3, watched.then_some(30)));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            stood,
            "the second press dropped the view (quit: {quit}, watched: {watched})"
        );
    }
}

/// A command that isn't the user's own act in the window, as another launch's ask for a new
/// window is, is no word of theirs on the close: the view stays kept for its Cancel.
#[gpui::test]
fn a_command_not_the_users_keeps_the_view(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    close(&shell, &host, cx, false);
    page_asks_to_stay(&shell, cx, 7);
    shell.update(cx, |shell, cx| shell.run(Command::NewWindow, "launch", cx));
    draw(&shell, cx);
    answer(&shell, cx, 7, false);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
}

/// A page of the task's the user watched after asking the close, before a page of theirs asked
/// to stay, goes back for the question as one watched at the close does: the core shows the
/// user's page to ask on, and Cancel brings the page watched back.
#[gpui::test]
fn a_page_watched_since_the_close_goes_back_for_the_question(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    draw(&shell, cx);
    close(&shell, &host, cx, false);
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    }));
    core_shows_task(&shell, cx, 30);
    draw(&shell, cx);
    host.take();
    host.shown.borrow_mut().clear();

    page_asks_to_stay(&shell, cx, 7);
    assert_eq!(
        *host.shown.borrow(),
        [1],
        "the question's page isn't the one the core shows"
    );
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    let asked = dialogs(&host);
    assert!(asked.contains(&"page dialog 10 7".to_string()), "{asked:?}");
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);

    answer(&shell, cx, 7, false);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );
}

/// Clear Archive with a task's view shown leaves it standing: with nothing to clear, while it
/// asks, and once the archive is cleared (#4202).
#[gpui::test]
fn clear_archive_leaves_the_tasks_view(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let shown = shell.read_with(cx, |shell, _| shell.task_shown());
    assert!(shown.is_some());
    shell.update(cx, |shell, cx| shell.run(Command::ClearArchive, "ui", cx));
    assert!(host.confirming.borrow().is_none(), "an empty archive asked");
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), shown);

    shell.update(cx, |shell, cx| {
        let tab = crate::shell::library::Archived {
            id: 7,
            url: "https://a.example/".into(),
            title: "A".into(),
            archived: 1000.,
            little: false,
        };
        shell.set_archive(vec![tab], cx);
        shell.run(Command::ClearArchive, "ui", cx);
    });
    let asked = host.confirming.borrow_mut().take();
    let (message, _, confirmed) = asked.expect("Clear Archive didn't ask");
    assert_eq!(message, "Clear the archive?");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        shown,
        "the view closed as it asked"
    );
    cx.update(|_, cx| confirmed(cx));
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        shown,
        "the view closed as the archive cleared"
    );
    assert!(
        host.shown.borrow().is_empty(),
        "the core was shown another Space"
    );
}

/// ⌘↩ on the page watched is the strip's Take Over while the agent drives it, which gives the
/// page the keyboard, and Hand Back once the user has it, which takes it back for the UI. ⌘. is
/// Stop.
#[gpui::test]
fn the_strip_takes_over_hands_back_and_stops(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let status = |held: Vec<(u64, SharedString)>| {
        let doing = "Typed · flights.example".into();
        vec![Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing,
            held,
        }]
    };
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(Vec::new()), cx);
        shell.agent_acted("task-1".into(), 30, cx);
    });
    host.take();
    assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    assert_eq!(host.take(), ["stop agent task-1"]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::BackToPinnedUrl, "ui", cx)
    });
    assert_eq!(host.take(), ["take over task-1 30", "focus page"]);

    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(vec![(30, "".into())]), cx)
    });
    shell.update(cx, |shell, cx| {
        shell.run(Command::BackToPinnedUrl, "page", cx)
    });
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );
}

/// A page the user took over says so in the same words under it and at the end of its row at the
/// sidebar's foot, while the task works on or waits for them elsewhere; the page it handed them
/// has the bar's own title, which names the agent, and its row's own word.
#[gpui::test]
fn a_taken_over_pages_row_ends_in_its_bars_title(cx: &mut TestAppContext) {
    use crate::agents::task::{Card, Dot, Word};
    use crate::shell::task_cards::PageWord;
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let card = |dot: Dot| Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight".into(),
        status: SharedString::default(),
        steps: Vec::new(),
        last_site: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        pages: vec![30, 31],
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
    };
    let hold = |why: &str, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let status = Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Typed · flights.example".into(),
                held: vec![(30, why.to_string().into())],
            };
            shell.set_agent_status(vec![status], cx);
        });
    };
    hold("", cx);
    for dot in [Dot::Working, Dot::Waiting] {
        shell.read_with(cx, |shell, _| {
            let row = shell.page_word(&card(dot), 30);
            assert_eq!(row, PageWord::Driving);
            assert_eq!(shell.task_strip_words(30).0, row.label());
        });
    }
    hold("Sign in", cx);
    shell.read_with(cx, |shell, _| {
        let row = shell.page_word(&card(Dot::Waiting), 30);
        assert_eq!(row, PageWord::NeedsYou);
        assert_ne!(shell.task_strip_words(30).0, PageWord::Driving.label());
    });
}

/// A press in the agent's page watched takes it over, as Take Over did, and the press is the page's
/// (#1080): the Agents hear the same take over, so nothing else moves. Pressed again while the user
/// holds it, nothing more happens; ⌘↩ then hands it back. A press in the page beside it in its
/// split view takes that tab over, not the one watched (#3243), and one in another agent's tab, or
/// with no agent at work, takes nothing.
#[gpui::test]
fn a_press_in_the_agents_page_takes_it_over_and_hand_back_gives_it_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let status = |held: Vec<(u64, SharedString)>| {
        vec![Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "Typed · flights.example".into(),
            held,
        }]
    };
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(Vec::new()), cx);
        shell.agent_acted("task-1".into(), 30, cx);
    });
    host.take();
    shell.update(cx, |shell, cx| shell.page_pressed(20, cx));
    assert!(
        host.take().is_empty(),
        "a press in another agent's tab took it from the window watching a task"
    );
    shell.update(cx, |shell, cx| shell.page_pressed(31, cx));
    assert_eq!(host.take(), ["take over task-1 31"]);
    shell.update(cx, |shell, cx| shell.page_pressed(30, cx));
    assert_eq!(host.take(), ["take over task-1 30"]);

    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(vec![(30, "".into()), (31, "".into())]), cx)
    });
    shell.update(cx, |shell, cx| shell.page_pressed(30, cx));
    assert!(
        host.take().is_empty(),
        "a press in a page the user holds took it again"
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::BackToPinnedUrl, "page", cx)
    });
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);

    shell.update(cx, |shell, cx| shell.set_agent_status(Vec::new(), cx));
    shell.update(cx, |shell, cx| shell.page_pressed(30, cx));
    assert!(
        host.take().is_empty(),
        "a press with no agent at work took a page"
    );
}

/// Whose tab it is decides, not where the agent acted last (#3503): a press in the tab it acted in
/// last takes it over, which ends its session there, and a press in the other tab of its split view
/// then takes that one over too. A press on a tab's dialog comes as a press in its tab.
#[gpui::test]
fn a_press_takes_over_each_of_the_agents_tabs_after_the_one_it_acted_in(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let status = |held: Vec<(u64, SharedString)>| {
        vec![Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "Typed · flights.example".into(),
            held,
        }]
    };
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(Vec::new()), cx);
        shell.agent_acted("task-1".into(), 31, cx);
    });
    host.take();
    shell.update(cx, |shell, cx| shell.page_pressed(31, cx));
    assert_eq!(host.take(), ["take over task-1 31"]);
    // The core ends the agent's session in the tab taken over, where it acted last.
    shell.update(cx, |shell, cx| {
        shell.agent_session_ended("task-1", 31, cx);
        shell.set_agent_status(status(vec![(31, "".into())]), cx);
    });
    host.take();
    shell.update(cx, |shell, cx| shell.page_pressed(30, cx));
    assert_eq!(
        host.take(),
        ["take over task-1 30"],
        "a press in the agent's other tab took nothing once the tab it acted in last was taken over"
    );
}

/// ⌘↩ reaches the strip before the agent's page does, as ⌘. does, so a form in the page can't
/// keep it as its own Send (#1080): Take Over while the agent drives, Hand Back once the user
/// holds the page. Without an agent's page, and from the UI's keys, where the command bar's ⌘↩
/// is its own, it's left alone.
#[gpui::test]
fn take_over_and_hand_back_come_before_the_agents_page(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let page_key = |cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run_agent_page_key("cmd-enter", cx))
    };
    let status = |held: Vec<(u64, SharedString)>| {
        vec![Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "Typed · flights.example".into(),
            held,
        }]
    };
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(Vec::new()), cx);
        shell.agent_acted("task-1".into(), 30, cx);
    });
    host.take();
    assert!(!shell.update(cx, |shell, cx| shell.run_agent_key("cmd-enter", cx)));
    assert!(page_key(cx));
    assert_eq!(host.take(), ["take over task-1 30", "focus page"]);

    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(vec![(30, "".into())]), cx)
    });
    assert!(page_key(cx));
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);

    shell.update(cx, |shell, cx| shell.set_agent_status(Vec::new(), cx));
    host.take();
    assert!(!page_key(cx), "⌘↩ was kept from a page no agent works in");
    assert!(host.take().is_empty());
}

/// Under Wayland GPUI gets the key the page had too, after the app answered it, and Hand Back has
/// given the UI the keyboard by then: that copy of ⌘↩ isn't the UI's Back to Pinned URL as well,
/// which would leave the agent's Space. A later ⌘↩ in the UI is.
#[cfg(target_os = "linux")]
#[gpui::test]
fn hand_back_from_the_page_is_one_press_in_the_ui_too(cx: &mut TestAppContext) {
    cx.update(crate::shell::commands::bind_keys);
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(
            vec![Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Typed · flights.example".into(),
                held: vec![(30, "".into())],
            }],
            cx,
        )
    });
    host.take();
    assert!(shell.update(cx, |shell, cx| shell.run_agent_page_key("cmd-enter", cx)));
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);
    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("cmd-enter");
    assert!(host.take().is_empty(), "the page's ⌘↩ ran in the UI too");
    cx.simulate_keystrokes("cmd-enter");
    assert!(!host.take().is_empty(), "a ⌘↩ of the UI's own did nothing");
}

/// The strip under an agent's page shows its speaker only while the page plays, crossed while it's
/// muted, as an agent's tabs start (#1080), and the speaker's click unmutes it as a row's does.
#[gpui::test]
fn the_strips_speaker_shows_while_the_page_plays(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let speaker = |cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.strip_sound(30, cx).is_some())
    };
    assert!(!speaker(cx), "a silent page has a speaker");
    let playing = Page {
        url: "https://flights.example/".into(),
        audible: true,
        muted: true,
        ..Default::default()
    };
    shell.update(cx, |shell, cx| shell.set_page(30, playing, cx));
    assert!(speaker(cx), "a page playing muted has no speaker");
    shell.update(cx, |shell, cx| shell.toggle_mute(30, cx));
    assert_eq!(*host.muted.borrow(), [(30, false)]);
}

/// A scroll never takes the agent's page over: the wheel's buttons, 4 to 7 on X11, aren't presses,
/// and scrolling over the page area takes nothing, where every mouse button's press does.
#[gpui::test]
fn a_scroll_in_the_agents_page_never_takes_it_over(cx: &mut TestAppContext) {
    for button in [1, 2, 3, 8, 9] {
        assert!(crate::agents::press_takes_over(button), "button {button}");
    }
    for wheel in 4..=7 {
        assert!(
            !crate::agents::press_takes_over(wheel),
            "the wheel's {wheel}"
        );
    }
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(
            vec![Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Typed · flights.example".into(),
                held: Vec::new(),
            }],
            cx,
        );
        shell.agent_acted("task-1".into(), 30, cx);
    });
    host.take();
    let over_the_page = point(px(700.), px(400.));
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: over_the_page,
        delta: gpui::ScrollDelta::Lines(point(0., -3.)),
        ..Default::default()
    });
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("take over")),
        "{calls:?}"
    );
}

/// An agent at work in a tab it then closes is at work nowhere: the page watched, its Space's other
/// tab, shows no bar for it, and Take Over offers nothing, until it acts in a tab again.
#[gpui::test]
fn an_agent_is_at_work_in_no_tab_once_it_closes_the_one_it_was_in(cx: &mut TestAppContext) {
    use crate::shell::agent_bar::Bar;
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(31, thumb, None, false, cx)
    }));
    core_shows_task(&shell, cx, 31);
    draw(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(
            vec![Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Clicked “Report”".into(),
                held: Vec::new(),
            }],
            cx,
        );
        shell.agent_acted("task-1".into(), 30, cx);
    });
    let at_work = |cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| match shell.agent_bar() {
            Some(Bar::Working { tab, .. }) => Some(tab),
            _ => None,
        })
    };
    assert_eq!(at_work(cx), Some(30));

    shell.update(cx, |shell, cx| {
        shell.set_tabs(3, Vec::new(), vec![31], Vec::new(), Folders::default(), cx)
    });
    draw(&shell, cx);
    assert_eq!(
        at_work(cx),
        None,
        "the bar still shows the agent at work in its closed tab"
    );
    host.take();
    shell.update(cx, |shell, cx| shell.agent_bar_primary(cx));
    assert!(host.take().is_empty(), "Take Over asked for the closed tab");

    shell.update(cx, |shell, cx| shell.agent_acted("task-1".into(), 31, cx));
    assert_eq!(at_work(cx), Some(31));
}

/// Escape on a task's page watched steps back to the conversation from the page's keyboard, which
/// a page the user clicked into has, as from the UI's, and stops neither the agent nor its load.
#[gpui::test]
fn escape_from_the_page_watched_steps_back(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    assert!(shell.update(cx, |shell, cx| shell.run_for_page(Command::Stop, cx)));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("stop")),
        "{calls:?}"
    );
}

/// The strip says who works and on what, that it finished and where, or that it stopped, and the
/// Signal ring goes round the page only while the task works: its chips are Stop while it works
/// and Open once it left the user a tab.
#[gpui::test]
fn the_strip_says_what_the_task_does(cx: &mut TestAppContext) {
    use crate::agents::task::{Card, Dot, Word};
    let (shell, _, cx) = shell_with_task(cx);
    let card = |dot: Dot, status: &str| Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight".into(),
        status: status.to_string().into(),
        steps: Vec::new(),
        last_site: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        pages: Vec::new(),
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
    };
    shell.update(cx, |shell, cx| {
        shell.set_page(
            30,
            Page {
                url: "https://www.gulbenkian.pt/tickets".into(),
                ..Page::default()
            },
            cx,
        );
    });
    let says = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let (title, detail) = shell.task_strip_words(30);
            (title, detail, shell.task_working())
        })
    };
    // The strip is of the card whose conversation the area shows.
    still(cx);
    let working = card(Dot::Working, "Clicking “Tickets”");
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![working], cx);
        shell.show_task("task-1".into(), cx);
    });
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    }));
    core_shows_task(&shell, cx, 30);
    draw(&shell, cx);
    let expected = (
        "Claude is working".to_string(),
        "Clicking “Tickets”".to_string(),
        true,
    );
    assert_eq!(says(&shell, cx), expected);

    let mut done = card(Dot::Done, "Tickets for two museums");
    done.open = vec![(30, "gulbenkian.pt".into()), (31, "maat.pt".into())];
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![done.clone()], cx));
    assert_eq!(
        says(&shell, cx),
        ("Claude finished".into(), "gulbenkian.pt".into(), false)
    );
    // Of the pages it left, the strip opens the page shown alone, and has nothing once it moved.
    let open = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.task_strip_open(30))
    };
    assert_eq!(open(&shell, cx).as_deref(), Some("Open gulbenkian.pt"));
    done.open.remove(0);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![done], cx));
    assert_eq!(open(&shell, cx), None);

    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(Dot::Ended, "Stopped")], cx)
    });
    assert_eq!(says(&shell, cx), ("Stopped".into(), "".into(), false));
    // An ended task's strip says the one word its card does: why it ended, never "Stopped" for
    // one the user didn't stop.
    for (word, says_it) in [
        (Word::Interrupted, "Interrupted"),
        (Word::AtLimit, "Claude at limit"),
    ] {
        let ended = Card {
            word,
            ..card(Dot::Ended, "")
        };
        shell.update(cx, |shell, cx| shell.set_task_cards(vec![ended], cx));
        assert_eq!(says(&shell, cx), (says_it.into(), "".into(), false));
    }
}

/// While a task's page is watched, the URL field shows its address, and Copy URL, Copy URL as
/// Markdown, the Site Controls and Edit URL act on it, not on the user's tab under it.
#[gpui::test]
fn the_url_field_is_the_page_watcheds(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    shell.update(cx, |shell, cx| {
        let page = |url: &str| Page {
            url: url.into(),
            ..Page::default()
        };
        shell.set_page(10, page("https://en.wikipedia.org/wiki/Lisbon"), cx);
        shell.set_page(30, page("https://www.gulbenkian.pt/tickets"), cx);
        shell.set_title(30, "Tickets".into(), cx);
    });
    watching(&shell, &host, cx);
    let field = shell.read_with(cx, |shell, _| shell.url(shell.field_tab()));
    assert_eq!(field.as_ref(), "https://www.gulbenkian.pt/tickets");
    shell.update(cx, |shell, cx| {
        shell.run(Command::CopyUrl, "ui", cx);
        shell.run(Command::CopyMarkdownLink, "ui", cx);
        shell.run(Command::SiteControls, "ui", cx);
    });
    let copied = host.copied.borrow().clone();
    assert_eq!(
        copied,
        [
            "https://www.gulbenkian.pt/tickets",
            "[Tickets](https://www.gulbenkian.pt/tickets)"
        ]
    );
    assert_eq!(
        host.site_controls.borrow().last().map(|(tab, _)| *tab),
        Some(30)
    );
    // Edit URL opens the bar on the page watched, which stays in place under it.
    shell.update(cx, |shell, cx| shell.run(Command::EditUrl, "ui", cx));
    assert_eq!(*host.bar_edits.borrow(), [Some(30)]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30))),
        "it left the page"
    );

    // Back in the conversation, the field shows no page, as nothing in the page area is one.
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    assert_eq!(shell.read_with(cx, |shell, _| shell.field_tab()), 0);
}

/// While a task's conversation is in the page area, no page is: the URL field reads as an empty
/// card's, with the command bar's placeholder, Copy URL copies nothing and Edit URL edits no tab,
/// and the Space's tab gives up its selected plate, which the task's card has. Back on the tab,
/// both are the tab's again.
#[gpui::test]
fn the_conversation_shows_no_page_in_the_url_field_or_the_rows(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://en.wikipedia.org/wiki/Lisbon".into(),
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    let words =
        |shell: &Shell, cx: &gpui::App| shell.url_field_words(&shell.url(shell.field_tab()), cx).0;
    assert_eq!(
        shell.read_with(cx, |shell, cx| words(shell, cx)).as_ref(),
        "en.wikipedia.org"
    );
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    shell.read_with(cx, |shell, cx| {
        assert_eq!(
            words(shell, cx).as_ref(),
            crate::command_bar::PLACEHOLDER_TEXT
        );
        assert!(
            shell.selected_row().is_none(),
            "the Space's tab kept its plate"
        );
    });
    shell.update(cx, |shell, cx| {
        shell.run(Command::CopyUrl, "ui", cx);
        shell.run(Command::EditUrl, "ui", cx);
    });
    assert!(
        host.copied.borrow().is_empty(),
        "Copy URL copied the user's tab under the conversation"
    );
    assert_eq!(*host.bar_edits.borrow(), [None]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None)),
        "Edit URL left it"
    );

    shell.update(cx, |shell, cx| shell.close_task(cx));
    shell.read_with(cx, |shell, cx| {
        assert_eq!(words(shell, cx).as_ref(), "en.wikipedia.org");
        assert!(
            shell.selected_row() == Some(crate::shell::Row::Tab(10)),
            "the tab didn't get its plate back"
        );
    });
}

/// ⌘R and Back act on the page watched; ⌘T opens the bar over the conversation, the core showing
/// the user's Space again first, so a tab it opens is the user's.
#[gpui::test]
fn page_commands_act_on_the_page_watched(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::Reload, "ui", cx);
        shell.run(Command::Back, "ui", cx);
    });
    assert_eq!(host.take(), ["reload 30", "back 30"]);
    shell.update(cx, |shell, cx| shell.run(Command::NewTab, "ui", cx));
    assert_eq!(*host.shown.borrow(), [1]);
    assert_eq!(host.command_bars.borrow().len(), 1);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
}

/// The task's Space going, or its page watched closing, leaves the area or the page.
#[gpui::test]
fn the_area_follows_the_task_going(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(3, Vec::new(), vec![31], Vec::new(), Folders::default(), cx);
    });
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    assert_eq!(*host.shown.borrow(), [1]);
    core_shows_users(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
            ],
            cx,
        );
    });
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// The agent's pointer comes to an element it acts on in a task's page watched, and not in its
/// other tab or while only the conversation shows. It goes a moment after the agent's last action.
#[gpui::test]
fn the_agents_pointer_shows_where_it_acts_in_the_page_watched(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    let element = Bounds::new(point(px(40.), px(60.)), size(px(120.), px(30.)));
    let point_at = |shell: &Entity<Shell>, cx: &mut VisualTestContext, tab| {
        shell.update(cx, |shell, cx| {
            shell.agent_pointing(tab, "e3".into(), element, cx);
            shell.agent_pointer_shown()
        })
    };
    assert!(!point_at(&shell, cx, 30), "the user's page shows");
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    assert!(!point_at(&shell, cx, 30), "only the conversation shows");
    watching(&shell, &host, cx);
    assert!(!point_at(&shell, cx, 31), "the window doesn't show tab 31");
    assert!(point_at(&shell, cx, 30));
    cx.executor().advance_clock(Duration::from_secs(1));
    assert!(shell.update(cx, |shell, _| shell.agent_pointer_shown()));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert!(!shell.update(cx, |shell, _| shell.agent_pointer_shown()));
}

/// The strip under a page the user holds names its agent as the Agents name it, the name its card
/// has, which for an outside client is the one it gave, not its Space's.
#[gpui::test]
fn the_strip_names_the_agent_as_its_card_does(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let held = |name: &str| {
        vec![Status {
            agent: "task-1".into(),
            name: name.to_string().into(),
            space: 3,
            doing: "Typed · flights.example".into(),
            held: vec![(30, "Sign in to flights.example".into())],
        }]
    };
    let says = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.task_strip_words(30))
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(held("Zed"), cx));
    assert_eq!(
        says(&shell, cx),
        (
            "Zed needs you".to_string(),
            "Sign in to flights.example".to_string()
        )
    );
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(held("MCP client"), cx)
    });
    assert_eq!(says(&shell, cx).0, "MCP client needs you");
}

/// While a task works on several pages, the strip under each says that page's own step, never
/// another page's, and nothing under a page with no step yet; a task that says no page's steps
/// has its card's words under its page, as it did (#4102).
#[gpui::test]
fn the_strip_under_a_page_says_that_pages_own_step(cx: &mut TestAppContext) {
    use crate::agents::task::{Card, Dot, Word};
    let (shell, _host, cx) = shell_with_task(cx);
    still(cx);
    let card = |steps: &[(u64, &str)]| Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find two seats".into(),
        status: "Working · 2 helpers".into(),
        asks: false,
        dot: Dot::Working,
        word: Word::for_dot(Dot::Working),
        pages: Vec::new(),
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: (steps.iter())
            .map(|(tab, step)| (*tab, step.to_string().into()))
            .collect(),
        retry: false,
        fix: None,
        steps: Vec::new(),
        last_site: None,
    };
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(&[])], cx);
        shell.show_task("task-1".into(), cx);
    });
    let mut says = |steps: &[(u64, &str)], tab: u64| {
        shell.update(cx, |shell, cx| {
            shell.set_task_cards(vec![card(steps)], cx);
            shell.task_strip_words(tab).1
        })
    };
    let steps = [(30, "Reading tickets.example"), (31, "Clicking “Stalls”")];
    assert_eq!(says(&steps, 30), steps[0].1);
    assert_eq!(says(&steps, 31), steps[1].1);
    assert_eq!(says(&steps[1..], 30), "");
    assert_eq!(says(&[], 30), "Working · 2 helpers");
}

/// The strip under a page is that page's own, never the task's (#4102). While the task waits on
/// the user at a page it handed them, the strip under another says the task works there, with
/// that page's own step, its ring round it, and its Take Over takes that page and no other,
/// wherever the task acted last. Hand Back is only under the page the user holds, which has no
/// ring.
#[gpui::test]
fn the_strip_under_a_page_is_that_pages_own_while_the_task_waits_at_another(
    cx: &mut TestAppContext,
) {
    use crate::agents::task::{Card, Dot, Word};
    use crate::shell::page_ends::StripStop;
    let (shell, host, cx) = shell_with_task(cx);
    still(cx);
    let steps = [
        (30, "Reading tickets.example"),
        (31, "Handing you boxoffice.example"),
    ];
    let card = Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find two seats".into(),
        status: "Needs you · Sign in".into(),
        asks: false,
        dot: Dot::Waiting,
        word: Word::for_dot(Dot::Waiting),
        pages: Vec::new(),
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: (steps.iter())
            .map(|(tab, step)| (*tab, step.to_string().into()))
            .collect(),
        retry: false,
        fix: None,
        steps: Vec::new(),
        last_site: None,
    };
    let held = |tab: u64| {
        vec![Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "Read · tickets.example".into(),
            held: vec![(tab, "Sign in".into())],
        }]
    };
    // The page watched is 30; the task handed 31 to the user, where it acted last.
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card], cx);
        shell.show_task("task-1".into(), cx);
    });
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    }));
    core_shows_task(&shell, cx, 30);
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(held(31), cx);
        shell.agent_acted("task-1".into(), 31, cx);
    });
    draw(&shell, cx);
    host.take();
    let strip = |cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let (title, detail) = shell.task_strip_words(30);
            let stops = shell.ring_stops();
            let stop = |stop: StripStop| stops.contains(&Some(stop));
            let chips = (stop(StripStop::TakeOver), stop(StripStop::HandBack));
            (title, detail, chips, shell.task_drives(30))
        })
    };
    let working = ("Claude is working".to_string(), steps[0].1.to_string());
    assert_eq!(strip(cx), (working.0, working.1, (true, false), true));
    shell.update(cx, |shell, cx| shell.agent_bar_primary(cx));
    let calls = host.take();
    assert_eq!(calls[0], "take over task-1 30", "{calls:?}");
    assert!(!calls.iter().any(|call| call.ends_with(" 31")), "{calls:?}");

    // The page watched is the one the user holds.
    shell.update(cx, |shell, cx| shell.set_agent_status(held(30), cx));
    let needs = ("Claude needs you".to_string(), "Sign in".to_string());
    assert_eq!(strip(cx), (needs.0, needs.1, (false, true), false));
}

/// Quiet until it needs you (#1080): a card under the URL field only ever asks, so each has one
/// primary, the answer its agent waits on; the strip under a page watched has one only while the
/// user holds the page its card waits for, Hand Back, and none while the agent works, has done or
/// stopped.
#[test]
fn only_what_needs_the_user_has_a_primary() {
    use super::{StripChip, strip_chips};
    use crate::agents::task::Dot;
    use crate::shell::permission::Look;

    let primaries = |held, dot, open| -> Vec<StripChip> {
        (strip_chips(held, dot, open, false).into_iter())
            .filter(|(_, look)| *look == Look::Primary)
            .map(|(chip, _)| chip)
            .collect()
    };
    assert_eq!(
        primaries(true, Some(Dot::Waiting), false),
        [StripChip::HandBack],
        "a handoff the user holds"
    );
    for (held, dot, open, what) in [
        (false, Some(Dot::Working), false, "at work"),
        (true, Some(Dot::Working), false, "taken over, nothing asked"),
        (false, Some(Dot::Waiting), false, "waiting on another tab"),
        (false, Some(Dot::Done), true, "done, with Open"),
        (false, Some(Dot::Ended), false, "stopped"),
    ] {
        assert!(primaries(held, dot, open).is_empty(), "{what}");
    }
    assert_eq!(
        strip_chips(false, Some(Dot::Working), false, false),
        [(StripChip::Stop, Look::Quiet)],
        "at work, Stop alone and quiet: a press in the page takes it over"
    );
    assert_eq!(
        strip_chips(false, Some(Dot::Working), false, true),
        [
            (StripChip::TakeOver, Look::Quiet),
            (StripChip::Stop, Look::Quiet)
        ],
        "with the ring in the strip, Take Over is a stop of its own, before Stop"
    );
    assert_eq!(
        strip_chips(true, Some(Dot::Working), false, true),
        [(StripChip::HandBack, Look::Quiet)],
        "a page the user holds has no Take Over"
    );
}

/// An agent's page watched, which the user doesn't hold: the ring goes over the strip in its own
/// order, the task's card, back, the page, Take Over and Stop, and never into the page or onto
/// the Space's rows, where Return would swap in another page. The page's stop
/// names it as the agent's, Return on it does nothing, and Return on Take Over takes it over, as
/// ⌘↩ does (#3220).
#[gpui::test]
fn the_ring_never_enters_an_agents_page(cx: &mut TestAppContext) {
    use crate::shell::page_ends::StripStop;
    let (shell, host, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    // Tab with the window's keyboard rings the strip, staying with the page watched.
    assert!(shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx)));
    draw(&shell, cx);
    host.take();
    let at = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| (shell.keyed.is_some(), shell.keyed_strip))
    };
    let tab = |reverse: bool, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.key_tab(reverse, cx));
        draw(shell, cx);
        at(shell, cx)
    };
    assert_eq!(at(&shell, cx), (false, Some(StripStop::Header(1))));
    let went: Vec<_> = (0..5).map(|_| tab(false, &shell, cx)).collect();
    assert_eq!(
        went,
        [
            (false, Some(StripStop::Back)),
            (false, Some(StripStop::Page)),
            (false, Some(StripStop::TakeOver)),
            (false, Some(StripStop::Stop)),
            (false, Some(StripStop::Header(1))),
        ]
    );
    let back: Vec<_> = (0..5).map(|_| tab(true, &shell, cx)).collect();
    assert_eq!(
        back,
        [
            (false, Some(StripStop::Stop)),
            (false, Some(StripStop::TakeOver)),
            (false, Some(StripStop::Page)),
            (false, Some(StripStop::Back)),
            (false, Some(StripStop::Header(1))),
        ]
    );
    // Shift+Tab from the window rings the last stop, Stop.
    shell.update(cx, |shell, cx| {
        shell.keyed_strip = None;
        shell.tab_into_strip(true, cx)
    });
    assert_eq!(at(&shell, cx), (false, Some(StripStop::Stop)));
    tab(true, &shell, cx);
    assert_eq!(tab(true, &shell, cx), (false, Some(StripStop::Page)));
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("focus page")),
        "the ring gave the agent's page the keyboard: {calls:?}"
    );
    let name = shell.read_with(cx, |shell, _| {
        let (title, detail) = shell.task_strip_words(30);
        shell.strip_page_name(30, &title, &detail)
    });
    assert!(name.starts_with("Claude's page"), "{name}");
    assert!(name.ends_with("Take Over to use it yourself."), "{name}");
    shell.update(cx, |shell, cx| shell.key_strip_show(StripStop::Page, cx));
    assert!(
        host.take().is_empty(),
        "Return on the page's stop did something"
    );
    shell.update(cx, |shell, cx| {
        shell.key_strip_show(StripStop::TakeOver, cx)
    });
    assert_eq!(
        host.take().first().map(String::as_str),
        Some("take over task-1 30")
    );
}

/// A working task's card shown, its page watched as the agent acts there.
fn watching_working(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    use crate::agents::task::{Card, Dot, Word};
    still(cx);
    let card = Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight".into(),
        status: "Clicking “Tickets”".into(),
        steps: Vec::new(),
        last_site: None,
        asks: false,
        dot: Dot::Working,
        word: Word::for_dot(Dot::Working),
        pages: Vec::new(),
        tier: Default::default(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
    };
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card], cx);
        shell.show_task("task-1".into(), cx);
    });
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    }));
    core_shows_task(shell, cx, 30);
    shell.update(cx, |shell, cx| {
        let status = Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "Clicking “Tickets”".into(),
            held: Vec::new(),
        };
        shell.set_agent_status(vec![status], cx);
        shell.agent_acted("task-1".into(), 30, cx);
    });
    draw(shell, cx);
}

/// Return on the task's header does what a click on it does: with one of its pages watched it
/// goes back to the task's view, and the ring leaves the strip.
#[gpui::test]
fn return_on_the_task_header_does_what_its_click_does(cx: &mut TestAppContext) {
    use crate::shell::page_ends::StripStop;
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    assert!(shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx)));
    draw(&shell, cx);
    let stop = shell.read_with(cx, |shell, _| shell.keyed_strip);
    assert_eq!(stop, Some(StripStop::Header(1)));
    shell.update(cx, |shell, cx| {
        shell.key_strip_show(StripStop::Header(1), cx)
    });
    draw(&shell, cx);
    let (shown, stop) = shell.read_with(cx, |shell, _| (shell.task_shown(), shell.keyed_strip));
    assert_eq!(
        shown,
        Some((3, None)),
        "Return on the header didn't go back to its task's view"
    );
    assert_eq!(stop, None, "the ring stayed in the strip");
}

/// A page the user holds is theirs: Tab from the ring's back goes into it, and Tab out of it
/// rings the strip's next stop, Hand Back, the UI taking the keyboard.
#[gpui::test]
fn a_held_task_page_takes_tab_as_the_users_does(cx: &mut TestAppContext) {
    use crate::shell::page_ends::StripStop;
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        let status = Status {
            agent: "task-1".into(),
            name: "Claude".into(),
            space: 3,
            doing: "".into(),
            held: vec![(30, "".into())],
        };
        shell.set_agent_status(vec![status], cx);
    });
    assert!(shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx)));
    draw(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| shell.key_tab(false, cx));
    draw(&shell, cx);
    assert_eq!(host.take(), ["focus page first"]);
    // Later than the press that went in, which a page with nothing to focus hands back.
    let took = shell.update(cx, |shell, cx| {
        shell.page_ends.entered = None;
        shell.tab_focus_leaving(30, false, cx)
    });
    assert!(took, "the app didn't take the keyboard from the held page");
    let stop = shell.read_with(cx, |shell, _| shell.keyed_strip);
    assert_eq!(stop, Some(StripStop::HandBack));
}

/// The Library opened by its key from the page, then a task's page watched: the Library's key
/// closing it gives the window the keyboard, never the agent's page (#3313).
#[gpui::test]
fn a_key_closing_the_library_never_gives_an_agents_page_the_keyboard(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    let library = |shell: &Entity<Shell>, from: &str, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.run(Command::ViewLibrary, from, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        draw(shell, cx);
    };
    library(&shell, "page", cx);
    watching(&shell, &host, cx);
    assert!(shell.read_with(cx, |shell, _| shell.library_open));
    library(&shell, "ui", cx);
    assert!(
        !host.take().contains(&"focus page".to_owned()),
        "the agent's page got it"
    );
    let root = cx.update(|window, cx| shell.read(cx).focus_handle.is_focused(window));
    assert!(root, "the field kept the UI's keyboard");
}

/// Take Over comes with the ring and moves nothing else in the strip: back and Stop stay where
/// they are, and the words, the agent's dot with them, keep their place and give up the room
/// (#3220).
#[gpui::test]
fn take_over_arriving_moves_nothing_else(cx: &mut TestAppContext) {
    use crate::shell::page_ends::{STOPS_DRAWN, StripStop};
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    let drawn = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
        draw(shell, cx);
        STOPS_DRAWN.with(|drawn| drawn.borrow().clone())
    };
    let before = drawn(&shell, cx);
    assert!(
        !before.contains_key(&StripStop::TakeOver),
        "Take Over shows without the ring"
    );
    shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx));
    let after = drawn(&shell, cx);
    let take_over = after[&StripStop::TakeOver];
    for stop in [StripStop::Back, StripStop::Stop] {
        assert_eq!(
            before[&stop], after[&stop],
            "{stop:?} moved as Take Over came"
        );
    }
    let (words, room) = (before[&StripStop::Page], after[&StripStop::Page]);
    assert_eq!(
        words.origin, room.origin,
        "the words moved as Take Over came"
    );
    assert!(
        room.right() <= take_over.left(),
        "the words run under Take Over"
    );
    assert!(
        room.size.width < words.size.width,
        "the words didn't give up the room"
    );
}

/// Take Over shows while the ring is on any of the task's stops, the card, back, the page, Take
/// Over and Stop, so it is there before the ring reaches its neighbour; never with the ring on a
/// stop that isn't the task's, a tab's row or New Tab's, nor with no ring (#3220).
#[gpui::test]
fn take_over_shows_with_the_ring_on_the_tasks_stops_alone(cx: &mut TestAppContext) {
    use crate::shell::page_ends::{STOPS_DRAWN, StripStop};
    use crate::shell::sidebar_keys::NEW_TAB;
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    let shows = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
        draw(shell, cx);
        STOPS_DRAWN.with(|drawn| drawn.borrow().contains_key(&StripStop::TakeOver))
    };
    assert!(!shows(&shell, cx), "Take Over shows with no ring");
    shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx));
    let stops = [
        StripStop::Header(1),
        StripStop::Back,
        StripStop::Page,
        StripStop::TakeOver,
        StripStop::Stop,
    ];
    for stop in stops {
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.keyed_strip),
            Some(stop)
        );
        assert!(
            shows(&shell, cx),
            "Take Over hides with the ring on {stop:?}"
        );
        shell.update(cx, |shell, cx| shell.key_tab(false, cx));
    }
    let row = shell.read_with(cx, |shell, _| {
        shell.key_rows().into_iter().find(|row| *row != NEW_TAB)
    });
    let row = row.expect("the Space has no tab's row");
    for (row, name) in [(row, "a tab's row"), (NEW_TAB, "New Tab's row")] {
        shell.update(cx, |shell, cx| shell.key_row(row, cx));
        assert!(
            !shows(&shell, cx),
            "Take Over shows with the ring on {name}"
        );
    }
    shell.update(cx, |shell, cx| shell.leave_keys("test", cx));
    assert!(!shows(&shell, cx), "Take Over shows once the ring leaves");
}

/// Standing on a working task's page, its conversation beside it, the ring goes only to the strip's
/// stops it draws: Stop is the composer's there, so the ring never sits on one nobody can see.
#[gpui::test]
fn standing_on_the_page_the_ring_rings_only_what_the_strip_draws(cx: &mut TestAppContext) {
    use crate::shell::page_ends::{STOPS_DRAWN, StripStop};
    let (shell, _, cx) = shell_with_task(cx);
    watching_working(&shell, cx);
    shell.update(cx, |shell, cx| shell.set_talk_beside(true, cx));
    set_zoom(&shell, cx, Zoom::Rest);
    assert!(shell.read_with(cx, |shell, _| shell.task_standing()));
    shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx));
    let mut rung = Vec::new();
    while let Some(stop) = shell.read_with(cx, |shell, _| shell.keyed_strip) {
        // Round once.
        if rung.contains(&stop) {
            break;
        }
        STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
        draw(&shell, cx);
        assert!(
            STOPS_DRAWN.with(|drawn| drawn.borrow().contains_key(&stop)),
            "the ring is on {stop:?}, which the strip doesn't draw beside the conversation"
        );
        rung.push(stop);
        shell.update(cx, |shell, cx| shell.key_tab(false, cx));
    }
    assert!(
        rung.contains(&StripStop::TakeOver),
        "the ring missed Take Over: {rung:?}"
    );
}

/// A press on the card of the agent's page watched whose renderer died, its Reload for one, is the
/// user acting on the agent's tab: it takes the tab over, as a press in its page does, and the
/// Reload still loads the page again (#3433).
#[gpui::test]
fn a_press_on_the_crash_card_of_the_agents_page_takes_it_over(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(
            vec![Status {
                agent: "task-1".into(),
                name: "Claude".into(),
                space: 3,
                doing: "Typed · flights.example".into(),
                held: Vec::new(),
            }],
            cx,
        );
        shell.agent_acted("task-1".into(), 30, cx);
        shell.set_page(
            30,
            Page {
                url: "https://flights.example/".into(),
                trouble: crate::shell::Trouble::Crashed,
                ..Page::default()
            },
            cx,
        );
    });
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        shell.crash_reload_drawn.set(None);
        cx.notify();
    });
    cx.run_until_parked();
    host.take();
    let reload = shell
        .read_with(cx, |shell, _| shell.crash_reload_drawn.get())
        .expect("the watched page that died shows the card's Reload");
    let place = shell.read_with(cx, |shell, _| watched(shell.page_frame(shell.viewport).0));
    assert!(
        place.contains(&reload.center()),
        "{reload:?} is in the page's place {place:?}"
    );
    cx.simulate_click(reload.center(), gpui::Modifiers::none());
    assert_eq!(host.take(), ["take over task-1 30", "reload 30"]);
}

/// An agent's tab never takes the user's keyboard with its page's alert (#3461): split in its own
/// Space, which no user's split shares, and watched in the task area, neither pane's alert shows,
/// live or at rest. Only a sign-in shows there, which only the user may answer (#3662).
#[gpui::test]
fn an_agents_split_never_shows_its_pages_dialogs(cx: &mut TestAppContext) {
    use crate::shell::{DialogKind, PageDialog, Split};
    let (shell, host, cx) = shell_with_task(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![30, 31],
            ..Default::default()
        };
        shell.set_tabs(
            3,
            Vec::new(),
            vec![30, 31],
            vec![split],
            Folders::default(),
            cx,
        );
    });
    watching(&shell, &host, cx);
    let alert = |id| PageDialog {
        id,
        kind: DialogKind::Alert,
        title: "example.org says".into(),
        message: "Hello".into(),
        text: "".into(),
        accept: "OK".into(),
        cancel: "".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    };
    for tab in [30, 31] {
        shell.update(cx, |shell, cx| {
            shell.set_page_dialog(tab, Some(alert(tab)), cx)
        });
        draw(&shell, cx);
    }
    let shown: Vec<String> = (host.take().into_iter())
        .filter(|call| call.starts_with("page dialog ") && call != "page dialog none")
        .collect();
    assert!(shown.is_empty(), "{shown:?}");
}

/// The task's view stands where a page stands (#4205): with the sidebar hidden, ⌥ alone brings it
/// out over the view and sends it away again, the pointer moving along the window's edge brings
/// it out, and ⌘S docks it, the view staying up throughout. With the conversation alone and with
/// a page of the task's in place in it, in a narrow window and in full screen.
#[gpui::test]
fn the_hidden_sidebar_comes_out_over_a_tasks_view(cx: &mut TestAppContext) {
    let cases = [
        ("the conversation", false, false, false),
        ("a page in place", true, false, false),
        ("the conversation, narrow", false, true, false),
        ("a page in place, narrow", true, true, false),
        ("the conversation, full screen", false, false, true),
        ("a page in place, full screen", true, false, true),
    ];
    for (case, page, narrow, full_screen) in cases {
        let (shell, host, cx) = shell_with_task(cx);
        still(cx);
        if narrow {
            cx.simulate_resize(size(px(560.), px(600.)));
        }
        if full_screen {
            shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
        }
        match page {
            true => watching(&shell, &host, cx),
            false => {
                let view = conversation(cx);
                shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
                draw(&shell, cx);
            }
        }
        let shown = shell.read_with(cx, |shell, _| shell.task_shown());
        assert_eq!(shown, Some((3, page.then_some(30))), "{case}");
        let toggle = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "key", cx));
            draw(shell, cx);
        };
        let revealed = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.revealed)
        };
        toggle(&shell, cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.sidebar_hidden),
            "{case}: ⌘S didn't hide the sidebar"
        );

        shell.update(cx, |shell, cx| shell.option_tapped(cx));
        assert!(revealed(&shell, cx), "{case}: ⌥ brought no sidebar out");
        shell.update(cx, |shell, cx| shell.option_tapped(cx));
        assert!(!revealed(&shell, cx), "{case}: ⌥ again left it out");

        // Into the strip at the window's edge, then a few points along it, as the window hears it.
        draw(&shell, cx);
        cx.simulate_mouse_move(point(px(1.), px(300.)), None, gpui::Modifiers::none());
        cx.simulate_mouse_move(point(px(1.), px(306.)), None, gpui::Modifiers::none());
        cx.executor().advance_clock(crate::shell::REVEAL_WAIT * 2);
        cx.run_until_parked();
        assert!(
            revealed(&shell, cx),
            "{case}: the edge brought no sidebar out"
        );

        toggle(&shell, cx);
        shell.read_with(cx, |shell, _| {
            assert!(!shell.sidebar_hidden, "{case}: ⌘S didn't dock the sidebar");
            assert!(!shell.reveal_shown(), "{case}: the reveal stayed out");
            assert_eq!(shell.task_shown(), shown, "{case}: the task's view went");
        });
    }
}
