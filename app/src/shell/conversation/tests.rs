//! A task's card and its conversation in the page area, on a host that records what the shell asks
//! of the core.

use std::rc::Rc;

use gpui::{Entity, Focusable, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::Fold;

use crate::agents::runner::Cli;
use crate::agents::task::{Card, Conversation, Dot, Line, Opens, Trace, TraceItem, TraceStep};
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
        asks: false,
        dot: Dot::Working,
        answers: Vec::new(),
        question: None,
        open: None,
        retry: false,
        command: None,
    }
}

/// A step of a trace, under way or done.
fn step(key: usize, title: &str, running: bool) -> TraceStep {
    TraceStep {
        key,
        glyph: "icons/globe.svg",
        title: title.to_string().into(),
        description: "".into(),
        running,
        failed: false,
        opens: Opens::Nothing,
    }
}

/// Task-1's conversation as it works, `steps` steps in.
fn conversation(steps: usize) -> Conversation {
    let items = (0..steps)
        .map(|key| TraceItem::Step(step(key, "Opened skyscanner.net", key + 1 == steps)))
        .collect();
    Conversation {
        agent: "task-1".into(),
        name: "Claude",
        title: "Flight to Lisbon".into(),
        about: "Claude · Haiku · Working".into(),
        asked: "a flight to Lisbon next Friday".into(),
        page: None,
        lines: vec![Line::Trace(Trace { key: 0, running: true, took: None, items })],
        tabs: vec![(30, "skyscanner.net".into())],
        on: Some(30),
        stoppable: true,
        open: None,
        answers: Vec::new(),
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

/// The conversation the Agents send a window that shows it is made before the window's shell
/// changes: the core reads its pages from a window's shell, which can't be read while it's being
/// changed. Made inside the change, it took the app down once a task changed with its
/// conversation open.
#[gpui::test]
fn the_agents_send_a_shown_conversation_made_before_its_shell_changes(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    host.take();
    let weak = shell.downgrade();
    cx.update(|_, cx| {
        let reads = weak.clone();
        // As the core's Browser::tab reads a page's title and address from the window's shell.
        let made = move |agent: &str, cx: &gpui::App| {
            reads.upgrade()?.read(cx).talk_agent()?;
            (agent == "task-1").then(|| conversation(1))
        };
        super::super::send_agents(Vec::new(), vec![card()], made, &[weak.clone()], cx);
    });
    assert!(host.take().contains(&"task picture 30".to_string()), "the conversation wasn't sent");
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

    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(1)), cx));
    assert_eq!(host.take(), ["task picture 30"]);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(1)), cx));
    assert!(host.take().is_empty(), "pictured again with nothing changed");
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(2)), cx));
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
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(1)), cx));
    host.take();
    cx.simulate_keystrokes("escape");
    assert!(!host.take().iter().any(|line| line.starts_with("task ")), "Esc stopped it");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent(), None);
        assert_eq!(shell.task_shown(), None);
    });

    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent().as_deref(), Some("task-1")));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(1)), cx));
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
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(1)), cx));
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

    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(2)), cx));
    assert!(!host.take().contains(&"task picture 30".to_string()), "pictured under the page");
    // Esc steps back, though the reply field the conversation focused isn't drawn under the page.
    cx.simulate_keystrokes("escape");
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert_eq!(shell.task_shown(), Some((3, None))));
    assert!(host.take().contains(&"task picture 30".to_string()), "pictured once back");
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(cx.update(|window, _| reply.is_focused(window)), "the reply field has the keys back");
}

/// A trace shows open while its stretch is under way and folds once it's over, unless the user
/// opened or folded it, which holds across its end.
#[gpui::test]
fn a_trace_is_open_while_it_works_unless_the_user_says_otherwise(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation(2)), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let open = |cx: &mut VisualTestContext, running| {
        view.read_with(cx, |view, _| view.is_open(Fold::Trace(0), running))
    };
    assert!(open(cx, true), "under way, it shows its steps");
    assert!(!open(cx, false), "over, it folds by itself");

    view.update(cx, |view, cx| view.toggle(Fold::Trace(0), true, cx));
    assert!(!open(cx, true), "folded by the user while it works");
    assert!(!open(cx, false), "and still folded once it's over");
    view.update(cx, |view, cx| view.toggle(Fold::Trace(0), false, cx));
    assert!(open(cx, false), "opened by the user once it's over");
    assert!(open(cx, true));
}

/// The newest line stays in view as lines come while the user reads at the end; scrolled up to
/// read, they stay where they are.
#[gpui::test]
fn the_newest_line_stays_in_view_at_the_end(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let talk = |lines: usize| {
        let mut talk = conversation(1);
        talk.lines.extend((0..lines).map(|n| Line::Said(format!("Line {n}").into())));
        talk
    };
    let draw = |cx: &mut VisualTestContext| {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(80)), cx));
    draw(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let scrolled = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| (view.scroll.offset().y, view.scroll.max_offset().y))
    };
    let (offset, max) = scrolled(cx);
    assert!(max > px(0.), "the lines fit, so nothing scrolls");
    assert_eq!(offset, -max, "it opened away from its end");

    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(81)), cx));
    draw(cx);
    let (offset, max) = scrolled(cx);
    assert_eq!(offset, -max, "a new line went out of view");

    view.update(cx, |view, _| view.scroll.set_offset(point(px(0.), px(0.))));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(82)), cx));
    draw(cx);
    assert_eq!(scrolled(cx).0, px(0.), "it pulled the user away from what they read");

    // Left and opened again, after the task went on meanwhile, it opens at its newest line.
    cx.simulate_keystrokes("escape");
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(84)), cx));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(84)), cx));
    draw(cx);
    let (offset, max) = scrolled(cx);
    assert_eq!(offset, -max, "it opened again where the user had left it");
}
