//! A task's Space and the page area showing it, on a host that records what the shell asks of the
//! core. The core's answers are played back as it posts them: the Space shown, then the tab.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyView, AppContext, Bounds, Context, Entity, IntoElement, Pixels, Render, SharedString,
    TestAppContext, VisualTestContext, Window, div, point, px, size,
};

use super::{Zoom, pushed, watched};
use crate::agents::Status;
use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};

/// The user's Space 1 with tabs 10 and 11, shown with 10, Claude Code's own Space 2 with tab 20,
/// and Space 3 of the task task-1, run by Claude Code, with tabs 30 and 31.
fn shell_with_task(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    host.space_agents.borrow_mut().insert(2, "claude-code".into());
    host.space_agents.borrow_mut().insert(3, "task-1".into());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
                (3, "Find a flight".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
            ],
            cx,
        );
        shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_tabs(3, Vec::new(), vec![30, 31], Vec::new(), Folders::default(), cx);
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
    shell.read_with(cx, |shell, _| shell.viewport.width - shell.page_frame(shell.viewport).0.left())
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

/// A conversation open, and the task's tab 30 watched, at rest in place, with the core showing it.
fn watching(shell: &Entity<Shell>, host: &Recorder, cx: &mut VisualTestContext) {
    still(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    assert!(shell.update(cx, |shell, cx| shell.watch_task_page(30, thumb, None, cx)));
    core_shows_task(shell, cx, 30);
    draw(shell, cx);
    host.take();
    host.shown.borrow_mut().clear();
}

/// A task's Space is in none of the window's lists of Spaces: not the Spaces menu, ⌃1 to ⌃9, Move
/// to or Search Tabs. Claude Code's own Space still is, and the task's tabs are kept for its
/// conversation.
#[gpui::test]
fn a_tasks_space_is_never_listed(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    assert_eq!(host.menu_bar.borrow().spaces, ["Space 1", "Research"]);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(2), "ui", cx));
    assert!(host.shown.borrow().is_empty(), "no third Space for ⌃3");
    shell.update(cx, |shell, cx| shell.tab_menu(10, point(px(100.), px(200.)), cx));
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    let titles: Vec<SharedString> = menu.into_iter().map(|(title, _)| title).collect();
    assert!(titles.contains(&"Research".into()), "{titles:?}");
    assert!(!titles.contains(&"Find a flight".into()), "{titles:?}");
    shell.update(cx, |shell, cx| {
        for tab in [11, 20, 30, 31] {
            let page = Page { url: format!("https://{tab}.test/").into(), ..Page::default() };
            shell.set_page(tab, page, cx);
        }
    });
    let searched = shell.read_with(cx, |shell, _| shell.open_tabs());
    let searched: Vec<u64> = searched.iter().map(|tab| tab.tab).collect();
    assert_eq!(searched, [11, 20], "the task's 30 and 31 aren't the user's to search");
    let bar = shell.read_with(cx, |shell, _| shell.bar_tabs());
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
    assert_eq!(shell.read_with(cx, |shell, _| (shell.space(), shell.selected())), (1, 10));
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
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, None)));
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, card);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(host.take(), ["focus page"]);
    assert!(host.shown.borrow().is_empty(), "the core showed the user's Space throughout");
}

/// With motion, the conversation springs in from the card's side, the page going the other way
/// at once with it, and leaves the same way.
#[gpui::test]
fn the_conversation_slides_in_and_back_out(cx: &mut TestAppContext) {
    let (shell, _, cx) = shell_with_task(cx);
    let view = conversation(cx);
    shell.update(cx, |shell, cx| shell.open_task(3, view, cx));
    let age = |shell: &Entity<Shell>, cx: &mut VisualTestContext, ago: Duration| {
        shell.update(cx, |shell, _| {
            let area = shell.task_area.as_mut().expect("open");
            let (from, _) = area.slide.expect("sliding");
            area.slide = Some((from, Instant::now() - ago));
        })
    };
    age(&shell, cx, Duration::from_millis(60));
    draw(&shell, cx);
    let (card, laid) = frames(&shell, cx);
    assert!(laid.left() > card.left() && laid.left() < card.left() + away(&shell, cx), "{laid:?}");
    age(&shell, cx, Duration::from_secs(1));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    assert!(shell.read_with(cx, |shell, _| shell.task_area.as_ref().unwrap().slide.is_none()));

    shell.update(cx, |shell, cx| shell.close_task(cx));
    age(&shell, cx, Duration::from_millis(60));
    draw(&shell, cx);
    assert!(frames(&shell, cx).1.left() > card.left());
    age(&shell, cx, Duration::from_secs(1));
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
    assert_eq!(card.size.height - laid.size.height, px(crate::tokens::shell::task_area::PAGE_ROOM));
    assert_eq!(shell.read_with(cx, |shell, _| (shell.space(), shell.selected())), (1, 10));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, Some(30))));

    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    assert_eq!(*host.shown.borrow(), [1], "the core shows the user's Space again");
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, None)));

    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, card);
    let calls = host.take();
    assert!(!calls.iter().any(|call| call.starts_with("stop")), "{calls:?}");
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
    shell.update(cx, |shell, cx| shell.watch_task_page(30, thumb, None, cx));
    assert_eq!(host.take(), ["select 30", "focus ui"]);
    core_shows_task(&shell, cx, 30);
    draw(&shell, cx);
    let (card, laid) = frames(&shell, cx);
    assert_eq!(laid, pushed(watched(card), away(&shell, cx)), "out of the window as it zooms");
    shell.update(cx, |shell, _| {
        let page = shell.task_area.as_mut().unwrap().page.as_mut().unwrap();
        page.zoom = Zoom::Out(Instant::now() - Duration::from_secs(1));
    });
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, watched(card), "in place");

    shell.update(cx, |shell, cx| shell.task_page_back(cx));
    let calls = host.take();
    assert_eq!(calls, ["picture 30", "focus ui"]);
    let request = shell.read_with(cx, |shell, _| {
        match shell.task_area.as_ref().unwrap().page.as_ref().unwrap().zoom {
            Zoom::Asked(request) => request,
            zoom => panic!("{zoom:?}"),
        }
    });
    assert!(host.shown.borrow().is_empty(), "the page stays till its picture comes");
    shell.update(cx, |shell, cx| shell.page_pictured(request, None, cx));
    assert_eq!(*host.shown.borrow(), [1]);
    core_shows_users(&shell, cx);
    draw(&shell, cx);
    assert_eq!(frames(&shell, cx).1, pushed(card, away(&shell, cx)));
    shell.update(cx, |shell, _| {
        let page = shell.task_area.as_mut().unwrap().page.as_mut().unwrap();
        page.zoom = Zoom::In(Instant::now() - Duration::from_secs(1));
    });
    draw(&shell, cx);
    assert!(shell.read_with(cx, |shell, _| shell.task_area.as_ref().unwrap().page.is_none()));
}

/// Selecting any of the user's tabs, the one selected too, or another Space leaves the area and
/// shows it. The window's own commands, such as ⌘S, leave the area as it is.
#[gpui::test]
fn selecting_a_tab_or_a_space_leaves_the_area(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
    assert!(shell.read_with(cx, |shell, _| shell.task_shown().is_some()));
    shell.update(cx, |shell, cx| shell.select(10, cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(host.take(), ["select 10"], "shown again, as the core shows the task's tab");
    core_shows_users(&shell, cx);

    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(1), "ui", cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    assert_eq!(*host.shown.borrow(), [1, 2], "the user's Space back, then the second");

    let view = conversation(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(1, cx);
        shell.open_task(3, view, cx);
        shell.select(11, cx);
    });
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// ⌘↩ on the page watched is the strip's Take Over while the agent drives it, which gives the
/// page the keyboard, and Hand Back once the user has it, which takes it back for the UI. ⌘. is
/// Stop. Escape from the page stops neither the agent nor its load.
#[gpui::test]
fn the_strip_takes_over_hands_back_and_stops(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    let status = |held: Vec<(u64, SharedString)>| {
        let doing = "Typed · flights.example".into();
        vec![Status { agent: "task-1".into(), space: 3, doing, held }]
    };
    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(Vec::new()), cx);
        shell.agent_acted("task-1".into(), 30, cx);
    });
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "page", cx));
    assert_eq!(host.take(), Vec::<String>::new());
    assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    assert_eq!(host.take(), ["stop agent task-1"]);
    shell.update(cx, |shell, cx| shell.run(Command::BackToPinnedUrl, "ui", cx));
    assert_eq!(host.take(), ["take over task-1 30", "focus page"]);

    shell.update(cx, |shell, cx| shell.set_agent_status(status(vec![(30, "".into())]), cx));
    shell.update(cx, |shell, cx| shell.run(Command::BackToPinnedUrl, "page", cx));
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, Some(30))));
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
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, None)));
}

/// The task's Space going, or its page watched closing, leaves the area or the page.
#[gpui::test]
fn the_area_follows_the_task_going(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(3, Vec::new(), vec![31], Vec::new(), Folders::default(), cx);
    });
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), Some((3, None)));
    assert_eq!(*host.shown.borrow(), [1]);
    core_shows_users(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
            ],
            cx,
        );
    });
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}
