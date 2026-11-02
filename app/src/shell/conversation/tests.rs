//! A task's card and its conversation in the page area, on a host that records what the shell asks
//! of the core.

use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use crate::agents::runner::Cli;
use crate::agents::task::{Card, Conversation, Dot, Line};
use crate::shell::picture::PagePicture;
use crate::shell::{Command, Folders, Shell, Slot, recorder::Recorder};

fn card() -> Card {
    Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        cli: Cli::Claude,
        title: "Flight to Lisbon".into(),
        status: "Searching flights".into(),
        dot: Dot::Working,
        answers: Vec::new(),
        question: None,
        open: None,
        retry: false,
        command: None,
    }
}

fn conversation(doing: &str) -> Conversation {
    Conversation {
        agent: "task-1".into(),
        name: "Claude",
        title: "Flight to Lisbon".into(),
        asked: "a flight to Lisbon next Friday".into(),
        page: None,
        lines: vec![Line::Steps { last: "Opened a page · skyscanner.net".into(), count: 3 }],
        doing: Some(doing.to_string().into()),
        tabs: vec![(30, "skyscanner.net".into())],
        stoppable: true,
    }
}

/// The user's Space 1 with tabs 10 and 11, 10 shown, and task-1's Space 3 with tab 30, whose card
/// is at the top of Space 1's Today; a click on the card shows its conversation. Without motion,
/// so the area is in place at once.
fn shown(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    host.space_agents.borrow_mut().insert(3, "task-1".into());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, cx| {
        window.activate_window();
        cx.set_reduce_motion(true);
    });
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (3, "Flight to Lisbon".into(), None, 0, "".into(), "Claude".into(), "".into()),
            ],
            cx,
        );
        shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.set_tabs(3, Vec::new(), vec![30], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_task_cards(vec![card()], cx);
        shell.slides.clear();
    });
    cx.run_until_parked();
    let at = shell.read_with(cx, |shell, _| {
        (shell.drawn_list_top() + shell.layout()[&Slot::Task(1)] + 12., shell.width() / 2.)
    });
    host.take();
    cx.simulate_click(point(px(at.1), px(at.0)), Modifiers::none());
    cx.run_until_parked();
    (shell, host, cx)
}

/// A click on the card shows its conversation in the page area, with the reply field's keyboard;
/// its pages are pictured only while it shows, and again as what it does changes. Return sends
/// the reply and ⌘. stops the task.
#[gpui::test]
fn the_card_shows_its_conversation(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    assert!(host.take().contains(&"show task task-1".to_string()));
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent().as_deref(), Some("task-1"));
        assert_eq!(shell.task_shown(), Some((3, None)));
    });

    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
    assert_eq!(host.take(), ["task picture 30"]);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
    assert!(host.take().is_empty(), "pictured again with nothing changed");
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Clicked")), cx));
    assert_eq!(host.take(), ["task picture 30"]);

    cx.simulate_input("Ryanair");
    cx.simulate_keystrokes("enter");
    assert!(host.take().contains(&r#"task task-1 Reply("Ryanair")"#.to_string()));
    assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    assert!(host.take().contains(&"task task-1 Stop".to_string()));
}

/// Esc steps back to the Space's tab and never stops the task, its pictures let go; a second click
/// shows it again, pictured again, and ⌘W then closes the card, dismissing the task.
#[gpui::test]
fn escape_steps_back_and_command_w_dismisses(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
    host.take();
    cx.simulate_keystrokes("escape");
    assert!(!host.take().iter().any(|line| line.starts_with("task ")), "Esc stopped it");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent(), None);
        assert_eq!(shell.task_shown(), None);
    });

    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent().as_deref(), Some("task-1")));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
    assert!(host.take().contains(&"task picture 30".to_string()), "pictured again");
    shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
    assert!(host.take().contains(&"task task-1 Dismiss".to_string()));
    shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent(), None));
}

/// A picture's click zooms its page out of it, at the picture's place and with its image; what
/// the task does meanwhile isn't pictured under the page, but is once the page is back.
#[gpui::test]
fn a_picture_shows_its_page(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
    let request = crate::shell::picture::next_request() - 1;
    let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
    shell.update(cx, |shell, cx| shell.page_pictured(request, picture, cx));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let (tab, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    assert_eq!(tab, 30);
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    assert!(host.take().contains(&"select 30".to_string()));
    shell.read_with(cx, |shell, _| assert_eq!(shell.task_shown(), Some((3, Some(30)))));
    let zoomed = shell.read_with(cx, |shell, _| shell.task_page_drawn());
    assert_eq!(zoomed, Some((3, 30)));

    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Clicked")), cx));
    assert!(!host.take().contains(&"task picture 30".to_string()), "pictured under the page");
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "ui", cx));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert_eq!(shell.task_shown(), Some((3, None))));
    assert!(host.take().contains(&"task picture 30".to_string()), "pictured once back");
}
