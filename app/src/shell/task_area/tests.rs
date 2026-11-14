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
    assert!(shell.update(cx, |shell, cx| shell.watch_task_page(30, thumb, None, cx)));
    core_shows_task(shell, cx, 30);
    draw(shell, cx);
    host.take();
    host.shown.borrow_mut().clear();
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
    shell.update(cx, |shell, cx| shell.watch_task_page(30, thumb, None, cx));
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

/// Selecting any of the user's tabs, the one selected too, or another Space leaves the area and
/// shows it, and so does a quit. The window's own commands, such as ⌘S, leave the area as it is.
#[gpui::test]
fn selecting_a_tab_or_a_space_leaves_the_area(cx: &mut TestAppContext) {
    let (shell, host, cx) = shell_with_task(cx);
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
    assert!(shell.read_with(cx, |shell, _| shell.task_shown().is_some()));
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

    // A quit keeps the user's Space in the session, as the core shows it again first.
    watching(&shell, &host, cx);
    shell.update(cx, |shell, cx| shell.run(Command::Quit, "ui", cx));
    assert_eq!(*host.shown.borrow(), [1]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
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

/// A press in the agent's page watched takes it over, as Take Over did, and the press is the page's
/// (#1080): the Agents hear the same take over, so nothing else moves. Pressed again while the user
/// holds it, nothing more happens; ⌘↩ then hands it back. The host hears the pages' presses only
/// while an agent works.
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
    assert_eq!(host.take(), ["hear page presses true"]);
    shell.update(cx, |shell, cx| shell.page_pressed(cx));
    assert_eq!(host.take(), ["take over task-1 30"]);

    shell.update(cx, |shell, cx| {
        shell.set_agent_status(status(vec![(30, "".into())]), cx)
    });
    shell.update(cx, |shell, cx| shell.page_pressed(cx));
    assert!(
        host.take().is_empty(),
        "a press in a page the user holds took it again"
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::BackToPinnedUrl, "page", cx)
    });
    assert_eq!(host.take(), ["hand back 30", "focus ui"]);

    shell.update(cx, |shell, cx| shell.set_agent_status(Vec::new(), cx));
    assert_eq!(host.take(), ["hear page presses false"]);
    shell.update(cx, |shell, cx| shell.page_pressed(cx));
    assert!(
        host.take().is_empty(),
        "a press with no agent at work took a page"
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
    assert!(shell.update(cx, |shell, cx| shell.watch_task_page(31, thumb, None, cx)));
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
    use crate::agents::task::{Card, Dot};
    let (shell, _, cx) = shell_with_task(cx);
    let card = |dot: Dot, status: &str| Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Find a flight".into(),
        status: status.to_string().into(),
        asks: false,
        dot,
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        retry: false,
        command: None,
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
    assert!(shell.update(cx, |shell, cx| shell.watch_task_page(30, thumb, None, cx)));
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

/// Quiet until it needs you (#1080): a card under the URL field only ever asks, so each has one
/// primary, the answer its agent waits on; the strip under a page watched has one only while the
/// user holds the page its card waits for, Hand Back, and none while the agent works, has done or
/// stopped.
#[test]
fn only_what_needs_the_user_has_a_primary() {
    use super::{StripChip, strip_chips};
    use crate::agents::Question;
    use crate::agents::task::Dot;
    use crate::shell::agent_question::buttons;
    use crate::shell::permission::Look;

    let question = |handoff: Option<u64>, back: bool| Question {
        id: 1,
        agent: "Claude Code".into(),
        from: "claude-code".into(),
        what: "open bank.example".into(),
        why: "It's a bank's site.".into(),
        handoff,
        back,
        ..Default::default()
    };
    for (asked, primary) in [
        (question(None, false), "Allow Once"),
        (question(Some(30), false), "Show Tab"),
        (question(Some(30), true), "Hand Back"),
    ] {
        let buttons = buttons(&asked);
        let primaries: Vec<_> = (buttons.iter())
            .filter(|button| button.look == Look::Primary)
            .map(|button| button.label)
            .collect();
        assert_eq!(primaries, [primary], "{asked:?}");
        assert_eq!(
            buttons.last().map(|button| button.look),
            Some(match asked.handoff {
                Some(_) => Look::Primary,
                None => Look::Text,
            }),
            "the primary is on the right, and Always Allow on a row of its own"
        );
    }

    let primaries = |held, dot, open| -> Vec<StripChip> {
        (strip_chips(held, dot, open).into_iter())
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
        strip_chips(false, Some(Dot::Working), false),
        [(StripChip::Stop, Look::Quiet)],
        "at work, Stop alone and quiet: a press in the page takes it over"
    );
}
