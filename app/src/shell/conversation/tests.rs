//! A task's card and its conversation in the page area, on a host that records what the shell asks
//! of the core.

use std::rc::Rc;

use gpui::{
    Entity, Focusable, FontWeight, Modifiers, TestAppContext, VisualTestContext, WindowTextSystem,
    point, px,
};

use super::Fold;

use crate::agents::task::ilo::{IloButton, IloKind, IloLine};
use crate::agents::task::{Card, Conversation, Dot, Line, Opens, Trace, TraceItem, TraceStep};
use crate::agents::tier::{Tier, Why};
use crate::shell::picture::PagePicture;
use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};
use crate::tokens::shell::conversation::ilo::TEXT_LINE as ILO_TEXT_LINE;
use crate::tokens::shell::conversation::{INSET, SITE, SITE_GAP, TRACE_LINE};
use crate::tokens::shell::task_area::BESIDE;
use crate::tokens::text;
use crate::tooltip::text_width;

fn card() -> Card {
    Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Flight to Lisbon".into(),
        status: "Searching flights".into(),
        asks: false,
        dot: Dot::Working,
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
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
        under: "".into(),
    }
}

/// Task-1's conversation as it works, `steps` steps in.
fn conversation(steps: usize) -> Conversation {
    let items = (0..steps)
        .map(|key| TraceItem::Step(step(key, "Opened skyscanner.net", key + 1 == steps)))
        .collect();
    Conversation {
        agent: "task-1".into(),
        name: "Claude".into(),
        title: "Flight to Lisbon".into(),
        about: "Claude · Haiku · Working".into(),
        progress: "".into(),
        asked: "a flight to Lisbon next Friday".into(),
        page: None,
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            items,
        })],
        tabs: vec![(30, "skyscanner.net".into())],
        on: Some(30),
        stoppable: true,
        open: None,
        answers: Vec::new(),
        writing: false,
        tier: Default::default(),
    }
}

/// The user's Space 1 with tabs 10 and 11, 10 shown, and task-1's Space 3 with tab 30, whose card
/// is at the top of Space 1's Today; a click on the card shows its conversation. Without motion,
/// so the area is in place at once.
fn shown(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    shown_with(card(), cx)
}

/// As `shown`, with `card` for task-1's.
fn shown_with(
    card: Card,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let agent = card.agent.clone();
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    host.space_agents.borrow_mut().insert(3, agent.clone());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, cx| {
        window.activate_window();
        cx.set_reduce_motion(true);
    });
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
                    3,
                    "Flight to Lisbon".into(),
                    None,
                    0,
                    "".into(),
                    "Claude".into(),
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
        shell.set_tabs(3, Vec::new(), vec![30], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_task_cards(vec![card], cx);
        shell.slides.clear();
    });
    cx.run_until_parked();
    let plate = shell
        .read_with(cx, |shell, _| shell.task_plate_drawn(&agent))
        .expect("the task's plate");
    host.take();
    cx.simulate_click(plate.center(), Modifiers::none());
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
        super::super::send_agents(
            Vec::new(),
            vec![card()],
            Vec::new(),
            made,
            &[weak.clone()],
            cx,
        );
    });
    assert!(
        host.take().contains(&"task picture 30".to_string()),
        "the conversation wasn't sent"
    );
}

/// A click on the notification a task's need posted opens its card's conversation, as the card's
/// click does, and leaves it open if it shows already, where the card's click would put it away.
#[gpui::test]
fn a_needs_notification_opens_its_task(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
    host.take();

    let host_needs = Rc::new(NoNeeds);
    cx.update(|_, cx| crate::needs_you::open(host_needs, cx));
    let mut waits = card();
    waits.dot = Dot::Waiting;
    waits.status = "Which seat?".into();
    cx.update(|_, cx| crate::needs_you::follow(&[waits], cx));
    let agent = cx
        .update(|_, cx| crate::needs_you::clicked(1, cx))
        .expect("its task");
    for _ in 0..2 {
        shell.update(cx, |shell, cx| shell.show_needed_task(agent.clone(), cx));
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, None))
        );
    }
    assert_eq!(
        host.take()
            .iter()
            .filter(|asked| *asked == "show task task-1")
            .count(),
        1
    );
}

/// A host for the needs that says ilo is in the background and posts nowhere.
struct NoNeeds;

impl crate::needs_you::Host for NoNeeds {
    fn badge(&self, _: usize) {}
    fn active(&self) -> bool {
        false
    }
    fn notify(&self, _: u64, _: &str, _: &str) {}
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

    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    assert_eq!(host.take(), ["task picture 30"]);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    assert!(
        host.take().is_empty(),
        "pictured again with nothing changed"
    );
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(2)), cx)
    });
    assert_eq!(host.take(), ["task picture 30"]);

    cx.simulate_input("Ryanair");
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&r#"task task-1 Reply("Ryanair")"#.to_string())
    );
    assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    assert!(host.take().contains(&"task task-1 Stop".to_string()));
}

/// An outside agent's session shows its steps one by one, with no ask, and in the reply field's
/// place a line that says to talk to it where it was started: the window keeps the keyboard, so
/// typing and Return send nothing, and ⌘. still stops it.
#[gpui::test]
fn a_sessions_conversation_has_no_reply_field(cx: &mut TestAppContext) {
    let session = Card {
        agent: "session-1".into(),
        name: "Claude Code".into(),
        title: "Claude Code".into(),
        replies: false,
        ..card()
    };
    let (shell, host, cx) = shown_with(session, cx);
    let talk = Conversation {
        agent: "session-1".into(),
        name: "Claude Code".into(),
        title: "Claude Code".into(),
        about: "Working".into(),
        progress: "".into(),
        asked: "".into(),
        lines: vec![
            Line::Step(step(0, "Read the page", false)),
            Line::Step(step(1, "Clicking “Tickets”", true)),
        ],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    // Its trace opens as it first draws, and shimmers from the next frame.
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let (reply, window) = cx.update(|window, cx| {
        let reply = view.read(cx).reply.focus_handle(cx).is_focused(window);
        (reply, shell.read(cx).focus_handle.is_focused(window))
    });
    assert!(!reply && window, "the keyboard isn't the window's");
    assert!(
        view.read_with(cx, |view, _| view.moving(false)),
        "the step under way doesn't shimmer"
    );
    host.take();
    cx.simulate_input("Ryanair");
    cx.simulate_keystrokes("enter");
    assert!(
        !host.take().iter().any(|line| line.contains("Reply")),
        "a reply went to a session"
    );
    assert!(shell.update(cx, |shell, cx| shell.run_agent_key("cmd-.", cx)));
    assert!(host.take().contains(&"task session-1 Stop".to_string()));
}

/// Esc steps back to the Space's tab and never stops the task, its pictures let go; a second click
/// shows it again, pictured again, and ⌘W then closes the card, dismissing the task.
#[gpui::test]
fn escape_steps_back_and_command_w_dismisses(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    host.take();
    cx.simulate_keystrokes("escape");
    assert!(
        !host.take().iter().any(|line| line.starts_with("task ")),
        "Esc stopped it"
    );
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent(), None);
        assert_eq!(shell.task_shown(), None);
    });

    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.talk_agent().as_deref(), Some("task-1"))
    });
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    assert!(
        host.take().contains(&"task picture 30".to_string()),
        "pictured again"
    );
    shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
    assert!(host.take().contains(&"task task-1 Dismiss".to_string()));
    shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent(), None));
}

/// A task with two pages opens on Pages, its grid, a picture each; a pick zooms its page out of
/// its picture, Esc puts it back in its picture on Pages, and Esc again goes to Talk, whose lines
/// show, and a third closes the task.
#[gpui::test]
fn two_pages_open_on_pages_and_esc_goes_back_to_talk(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "seatmap.example".into()),
    ];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, cx| shell.talk_on_pages(cx)),
        "two pages opened on Talk"
    );
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let tiles: Vec<u64> = view.read_with(cx, |view, _| {
        view.drawn.borrow().iter().map(|(tab, _)| *tab).collect()
    });
    assert_eq!(tiles, [30, 31]);
    let (_, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    assert!(host.take().contains(&"select 30".to_string()));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );

    cx.simulate_keystrokes("escape");
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    assert!(
        shell.read_with(cx, |shell, cx| shell.talk_on_pages(cx)),
        "the page went back to Talk"
    );
    shell.update(cx, |shell, cx| shell.task_step_back(cx));
    assert!(
        !shell.read_with(cx, |shell, cx| shell.talk_on_pages(cx)),
        "Esc stayed on Pages"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None))
    );
    shell.update(cx, |shell, cx| shell.task_step_back(cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// A picture's click zooms its page out of it, at the picture's place and with its image; what
/// the task does meanwhile isn't pictured under the page, but is once the page is back.
#[gpui::test]
fn a_picture_shows_its_page(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let request = crate::shell::picture::next_request() - 1;
    let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
    shell.update(cx, |shell, cx| shell.page_pictured(request, picture, cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    // One page opens on Talk, which has no pictures: Pages has them.
    assert!(!shell.read_with(cx, |shell, cx| shell.talk_on_pages(cx)));
    view.update(cx, |view, cx| view.show_pages(true, cx));
    cx.run_until_parked();
    let (tab, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    assert_eq!(tab, 30);
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    assert!(host.take().contains(&"select 30".to_string()));
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.task_shown(), Some((3, Some(30))))
    });
    let zoomed = shell.read_with(cx, |shell, _| shell.task_page_drawn());
    assert_eq!(zoomed, Some((3, 30)));

    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(2)), cx)
    });
    assert!(
        !host.take().contains(&"task picture 30".to_string()),
        "pictured under the page"
    );
    // Esc steps back, though the reply field the conversation focused isn't drawn under the page.
    cx.simulate_keystrokes("escape");
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.task_shown(), Some((3, None)))
    });
    assert!(
        host.take().contains(&"task picture 30".to_string()),
        "pictured once back"
    );
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(
        cx.update(|window, _| reply.is_focused(window)),
        "the reply field has the keys back"
    );
}

/// While the page watched goes back into its picture, the conversation under it takes a press on a
/// picture alone, which watches its page (#3496). The rest of it, which the user can't see yet,
/// takes none: Stop doesn't stop the task, a picture's speaker mutes nothing, and the reply field
/// doesn't take the keyboard.
#[gpui::test]
fn under_a_page_going_back_only_the_pictures_take_a_press(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "seatmap.example".into()),
    ];
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            3,
            Vec::new(),
            vec![30, 31],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_conversation(Some(talk), cx);
    });
    let playing = Page {
        url: "https://seatmap.example/".into(),
        audible: true,
        ..Default::default()
    };
    shell.update(cx, |shell, cx| shell.set_page(31, playing, cx));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let picture = |tab: u64, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            let drawn = view.drawn.borrow();
            drawn
                .iter()
                .find(|(each, _)| *each == tab)
                .expect("its picture")
                .1
        })
    };
    let (tickets, seatmap) = (picture(30, cx), picture(31, cx));
    let stop = view.read_with(cx, |view, _| view.stop_logged.get().expect("Stop"));
    let reply = view.read_with(cx, |view, _| {
        view.reply_drawn.get().expect("the reply field")
    });
    // The speaker starts the playing page's site line, under its picture.
    let speaker = point(
        seatmap.left() + px((text::SMALL + 1.) / 2.),
        seatmap.bottom() + px(SITE_GAP + SITE / 2.),
    );
    cx.simulate_click(tickets.center(), Modifiers::none());
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );
    cx.update(|window, cx| {
        cx.set_reduce_motion(false);
        window.focus(&shell.read(cx).focus_handle.clone(), cx);
    });
    shell.update(cx, |shell, cx| {
        shell.hold_task_page_going_back();
        cx.notify();
    });
    cx.run_until_parked();
    host.take();

    // The strip under the page covers the reply field but for its top, which the page covers.
    let field = point(reply.center().x, reply.top() + px(2.));
    for at in [stop.center(), speaker, field] {
        cx.simulate_click(at, Modifiers::none());
    }
    let asked = host.take();
    assert!(
        !asked.contains(&"task task-1 Stop".to_string()),
        "Stop under the page stopped the task: {asked:?}"
    );
    assert!(
        host.muted.borrow().is_empty(),
        "the speaker under the page muted it"
    );
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(
        !cx.update(|window, _| reply.is_focused(window)),
        "the reply field under the page took the keyboard"
    );

    cx.simulate_click(seatmap.center(), Modifiers::none());
    assert!(
        host.take().contains(&"select 31".to_string()),
        "the picture under the page didn't watch it"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(31)))
    );
}

/// A trace shows open while its stretch is under way and folds once it's over, unless the user
/// opened or folded it, which holds across its end.
#[gpui::test]
fn a_trace_is_open_while_it_works_unless_the_user_says_otherwise(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(2)), cx)
    });
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

/// A turn's same step on ten tabs is one line of its trace, folded, that opens to the ten tabs'
/// lines; opening it opens none of them, and a tab's line that failed opens on its own to why
/// (#1476).
#[gpui::test]
fn a_step_on_many_tabs_opens_to_each_tabs_line(cx: &mut TestAppContext) {
    use crate::tokens::shell::conversation::TRACE_LINE;
    let (shell, _, cx) = shown(cx);
    let tabs = (0..10)
        .map(|key| TraceStep {
            title: format!("cases.example/{key}").into(),
            failed: key == 3,
            opens: match key {
                3 => Opens::Text("There's no element e7 on the page.".into()),
                _ => Opens::Nothing,
            },
            ..step(key, "", false)
        })
        .collect();
    let group = TraceStep {
        title: "Clicked “Closed” on 10 pages".into(),
        description: "1 failed".into(),
        failed: true,
        opens: Opens::Steps(tabs),
        ..step(0, "", false)
    };
    let mut talk = conversation(1);
    let items = vec![TraceItem::Step(group)];
    talk.lines = vec![Line::Trace(Trace {
        key: 0,
        running: true,
        took: None,
        items,
    })];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };
    let height = |fold, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            view.is_open(fold, false)
                .then(|| view.heights.borrow()[&fold])
        })
    };
    draw(cx);
    assert_eq!(height(Fold::Group(0), cx), None, "it opened by itself");

    view.update(cx, |view, cx| view.toggle(Fold::Group(0), false, cx));
    draw(cx);
    assert_eq!(
        height(Fold::Group(0), cx),
        Some(10. * TRACE_LINE),
        "it didn't open to ten lines"
    );
    assert_eq!(
        height(Fold::Step(0), cx),
        None,
        "its first tab's line opened with it"
    );

    view.update(cx, |view, cx| view.toggle(Fold::Step(3), false, cx));
    draw(cx);
    assert!(
        height(Fold::Step(3), cx).is_some_and(|why| why > 0.),
        "the failed tab didn't say why"
    );
    let opened = height(Fold::Group(0), cx).unwrap();
    assert!(
        opened > 10. * TRACE_LINE,
        "why it failed shows under its tab: {opened}"
    );
}

/// A reply the user sends shows Thinking under it, in view though they'd scrolled up to read,
/// shimmering on frames while it shows, and still under Reduce Motion; the agent's first step
/// takes its place, and with nothing under way, nothing asks for a frame.
#[gpui::test]
fn a_sent_reply_thinks_until_the_first_step(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let mut talk = conversation(1);
    if let Line::Trace(trace) = &mut talk.lines[0] {
        (trace.running, trace.took, trace.items) = (
            false,
            Some(3),
            vec![TraceItem::Step(step(0, "Read the page", false))],
        );
    }
    talk.lines
        .extend((0..80).map(|n| Line::Said(format!("Line {n}").into())));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(talk.clone()), cx)
    });
    let draw = |cx: &mut VisualTestContext| {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };
    draw(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let moving =
        |still, cx: &mut VisualTestContext| view.read_with(cx, |view, _| view.moving(still));
    assert!(
        !moving(false, cx),
        "a frame is asked with nothing under way"
    );
    view.update(cx, |view, _| view.scroll.set_offset(point(px(0.), px(0.))));

    cx.simulate_input("What is my name?");
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&r#"task task-1 Reply("What is my name?")"#.to_string())
    );
    // The Agents' conversation on the reply, before the CLI says anything.
    talk.lines.extend([
        Line::You("What is my name?".into()),
        Line::Thinking {
            key: 1,
            failed: None,
        },
    ]);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(talk.clone()), cx)
    });
    draw(cx);
    let (offset, max) = view.read_with(cx, |view, _| {
        (view.scroll.offset().y, view.scroll.max_offset().y)
    });
    assert_eq!(offset, -max, "the Thinking is out of view");
    assert!(moving(false, cx), "Thinking doesn't shimmer");
    assert!(!moving(true, cx), "Thinking moves under Reduce Motion");

    talk.lines.pop();
    let first = Trace {
        key: 1,
        running: true,
        took: None,
        items: vec![TraceItem::Step(step(1, "Reading the page", true))],
    };
    talk.lines.push(Line::Trace(first));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(talk.clone()), cx)
    });
    draw(cx);
    assert!(moving(false, cx), "the step under way doesn't shimmer");
    if let Some(Line::Trace(trace)) = talk.lines.last_mut() {
        (trace.running, trace.took, trace.items) = (
            false,
            Some(1),
            vec![TraceItem::Step(step(1, "Read the page", false))],
        );
    }
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    draw(cx);
    assert!(!moving(false, cx), "a frame is asked once the turn is over");
}

/// The newest line stays in view as lines come while the user reads at the end; scrolled up to
/// read, they stay where they are.
#[gpui::test]
fn the_newest_line_stays_in_view_at_the_end(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let talk = |lines: usize| {
        let mut talk = conversation(1);
        talk.lines
            .extend((0..lines).map(|n| Line::Said(format!("Line {n}").into())));
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
        view.read_with(cx, |view, _| {
            (view.scroll.offset().y, view.scroll.max_offset().y)
        })
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
    assert_eq!(
        scrolled(cx).0,
        px(0.),
        "it pulled the user away from what they read"
    );

    // Left and opened again, after the task went on meanwhile, it opens at its newest line.
    cx.simulate_keystrokes("escape");
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(84)), cx));
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk(84)), cx));
    draw(cx);
    let (offset, max) = scrolled(cx);
    assert_eq!(offset, -max, "it opened again where the user had left it");

    // A shorter window keeps the end in view while the user reads there, and leaves a line above
    // it where it is.
    let size = cx.update(|window, _| window.viewport_size());
    cx.simulate_resize(gpui::size(size.width, size.height - px(200.)));
    draw(cx);
    let (offset, shorter) = scrolled(cx);
    assert!(shorter > max, "the window didn't get shorter");
    assert_eq!(offset, -shorter, "a shorter window lost the end");
    view.update(cx, |view, _| {
        view.scroll.set_offset(point(px(0.), px(-100.)))
    });
    cx.simulate_resize(gpui::size(size.width, size.height - px(260.)));
    draw(cx);
    assert_eq!(
        scrolled(cx).0,
        px(-100.),
        "it pulled the user away from what they read"
    );
}

/// The lines follow what comes at their end while the user reads there, on the curve it lands
/// on, at once as the conversation opens or with motion reduced, and not once they've scrolled up.
#[test]
fn the_lines_follow_only_while_the_user_reads_at_their_end() {
    use super::{Follows, follows};
    // grew, opening, pinned, moves
    assert_eq!(
        follows(false, false, true, true),
        Follows::Not,
        "nothing came"
    );
    assert_eq!(
        follows(true, true, false, true),
        Follows::AtOnce,
        "it opens at its end"
    );
    assert_eq!(
        follows(true, false, false, true),
        Follows::Not,
        "the user scrolled up to read"
    );
    assert_eq!(
        follows(true, false, true, false),
        Follows::AtOnce,
        "motion reduced"
    );
    assert_eq!(follows(true, false, true, true), Follows::Eased);
}

/// A step under way that shimmers while nothing else moves draws on the loading dots' clock, not
/// at every display frame, as each frame draws the whole window again (#1700).
#[gpui::test]
fn the_shimmer_draws_on_the_loading_dots_clock(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    let mut talk = conversation(1);
    if let Line::Trace(trace) = &mut talk.lines[0] {
        (trace.running, trace.took, trace.items) = (
            true,
            None,
            vec![TraceItem::Step(step(0, "Waiting on you", true))],
        );
    }
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    // Its trace opens as it first draws, and shimmers from the next frame.
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert!(
        view.read_with(cx, |view, _| view.moving(false)),
        "the step under way doesn't shimmer"
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.breath.pending()),
        "the shimmer isn't on the loading dots' clock"
    );
}

/// The reply field's "Reply to …" follows a switch between light and dark while the conversation
/// stays open, as its lines do, in the placeholder colour of the palette it's drawn in (#1659).
#[gpui::test]
fn the_reply_placeholder_follows_the_appearance(cx: &mut TestAppContext) {
    use crate::shell::Appearance;
    use crate::tokens::shell::FORM_PLACEHOLDER;
    let (shell, _, cx) = shown(cx);
    let mut colours = Vec::new();
    for appearance in [Appearance::Light, Appearance::Dark, Appearance::Light] {
        shell.update(cx, |shell, cx| {
            shell.run(Command::Appearance(appearance), "ui", cx)
        });
        cx.run_until_parked();
        let (drawn, meant) = shell.read_with(cx, |shell, cx| {
            let talk = shell.talk.as_ref().expect("the conversation");
            let reply = talk.view.read(cx).reply.read(cx).placeholder_hsla();
            (
                reply,
                gpui::Hsla::from(shell.palette.text(FORM_PLACEHOLDER)),
            )
        });
        assert_eq!(drawn, meant, "{appearance:?}'s placeholder");
        colours.push(drawn);
    }
    assert_ne!(
        colours[0], colours[1],
        "light and dark drew the same placeholder"
    );
    assert_eq!(colours[0], colours[2]);
}

/// A page the user holds from the agent says why under its picture, over the agent's step: taken
/// over, that they drive it; handed to them, what the agent asked of them (#1080).
#[test]
fn a_held_pages_picture_says_why() {
    use super::picture_status;
    let step = || Some("Clicking “Tickets”".into());
    assert_eq!(picture_status(None, step()), step());
    assert_eq!(picture_status(None, None), None);
    assert_eq!(
        picture_status(Some(&"".into()), step()),
        Some("You're driving".into())
    );
    assert_eq!(
        picture_status(Some(&"Sign in to github.com".into()), None),
        Some("Needs you · Sign in to github.com".into())
    );
}

/// Task-1's conversation with ilo's line that it stopped Claude at Place order, under Work on
/// pages, drawn, and its view.
fn stopped_at_order(
    cx: &mut TestAppContext,
) -> (
    Entity<Shell>,
    Rc<Recorder>,
    Entity<super::TalkView>,
    &mut VisualTestContext,
) {
    let (shell, host, cx) = shown(cx);
    let talk = Conversation {
        lines: vec![Line::Ilo(IloLine {
            key: 7,
            kind: IloKind::Stopped {
                tab: 30,
                at: "Place order".into(),
                why: Why::Order,
                tier: Tier::Work,
                count: 1,
            },
            settled: false,
        })],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    (shell, host, view, cx)
}

/// Where an ilo line's button was drawn, and since when it has stood there.
fn ilo_button(
    view: &Entity<super::TalkView>,
    key: (usize, IloButton),
    cx: &mut VisualTestContext,
) -> Option<(gpui::Bounds<gpui::Pixels>, std::time::Instant)> {
    view.read_with(cx, |view, _| view.ilo_drawn.borrow().get(&key).copied())
}

/// An ilo line is a kind of its own that only the app makes: what the agent writes, even ilo's
/// words and its buttons' names, is the agent's text and draws no button.
#[gpui::test]
fn an_agents_text_never_draws_an_ilo_lines_buttons(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let talk = Conversation {
        lines: vec![Line::Said(
            "ilo stopped Claude at “Place order”\n\nPlacing an order isn't part of Work on pages.\n\n\
             Take Over · Give Full access"
                .into(),
        )],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.ilo_drawn.borrow().is_empty()),
        "the agent's text drew an ilo line's button"
    );
    assert!(
        view.read_with(cx, |view, _| view.ilo_links.borrow().is_empty()),
        "the agent's text drew an ilo line's link"
    );
}

/// A site is never set in quotes on ilo's line, which is the app's to write; a page's own words
/// are, so they read as the page's.
#[gpui::test]
fn a_site_is_never_in_quotes_and_a_pages_words_are(_: &mut TestAppContext) {
    use crate::agents::task::ilo::SignedInWith;
    let stopped = |why| IloKind::Stopped {
        tab: 30,
        at: "seatmap.example".into(),
        why,
        tier: Tier::Work,
        count: 1,
    };
    let said = |kind: &IloKind| {
        let (own, name) = super::words::head(kind, "Claude");
        super::words::with_name(kind, &own, &name.unwrap())
    };
    assert_eq!(
        said(&stopped(Why::Fenced)),
        "stopped Claude from going to seatmap.example"
    );
    assert_eq!(
        said(&stopped(Why::OtherSite)),
        "stopped Claude from going to seatmap.example"
    );
    let signed_in = IloKind::SignedIn {
        tab: 30,
        site: "tickets.example".into(),
        with: SignedInWith::Saved("ana@example.com".into()),
    };
    assert_eq!(said(&signed_in), "signed in to tickets.example");
    let order = IloKind::Stopped {
        tab: 30,
        at: "Place order".into(),
        why: Why::Order,
        tier: Tier::Work,
        count: 1,
    };
    assert_eq!(
        said(&order),
        "stopped Claude at \u{201c}Place order\u{201d}"
    );
}

/// An ilo line's head is a sentence: where ilo is its subject the mark is its first word and a verb
/// of ilo's follows ("ilo stopped Claude at …"); where it isn't, a middle dot sets the mark off
/// and the sentence starts with a capital ("ilo · Claude needs this tab back").
#[gpui::test]
fn an_ilo_lines_head_reads_as_a_sentence_after_the_mark(_: &mut TestAppContext) {
    use super::words::head;
    use crate::agents::task::ilo::SignedInWith;
    // ilo's verbs. A new head that starts with another word fails here, to be read as a sentence.
    const VERBS: [&str; 4] = ["stopped", "signed", "handed", "resumed"];
    let every = [
        IloKind::Stopped {
            tab: 1,
            at: "Place order".into(),
            why: Why::Order,
            tier: Tier::Work,
            count: 1,
        },
        IloKind::Stopped {
            tab: 1,
            at: "seatmap.example".into(),
            why: Why::Fenced,
            tier: Tier::Work,
            count: 1,
        },
        IloKind::Yours { tab: 1 },
        IloKind::TabBack { tab: 1 },
        IloKind::SignedIn {
            tab: 1,
            site: "tickets.example".into(),
            with: SignedInWith::Extension("Keys".into()),
        },
        IloKind::TierChanged {
            from: Tier::Work,
            to: Tier::Full,
        },
        IloKind::Resumed { tier: Tier::Work },
        IloKind::KeptHere,
        IloKind::Gone,
        IloKind::NotKept,
        IloKind::KeepsNothing,
    ];
    for kind in &every {
        // Every kind is in the list above: a new one fails to compile here until it's added.
        match kind {
            IloKind::Stopped { .. }
            | IloKind::Yours { .. }
            | IloKind::TabBack { .. }
            | IloKind::SignedIn { .. }
            | IloKind::TierChanged { .. }
            | IloKind::Resumed { .. }
            | IloKind::KeptHere
            | IloKind::Gone
            | IloKind::NotKept
            | IloKind::KeepsNothing => {}
        }
        let (own, _) = head(kind, "Claude");
        let verb = VERBS.iter().any(|verb| own.split(' ').next() == Some(verb));
        let dotted = own
            .strip_prefix("\u{b7} ")
            .and_then(|rest| rest.chars().next())
            .is_some_and(char::is_uppercase);
        assert!(
            verb || dotted,
            "{kind:?} reads \"ilo {own}\": a head starts with one of ilo's verbs {VERBS:?}, or \
             with \"· \" and a capital"
        );
    }
}

/// Under a fence's refusal, the app's own link opens Settings on the Off limits list and does
/// nothing else: no task's access changes and no fence lifts. A refusal of another kind has none.
#[gpui::test]
fn a_fences_refusal_links_to_off_limits_and_changes_nothing(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let line = |key, why| {
        Line::Ilo(IloLine {
            key,
            kind: IloKind::Stopped {
                tab: 30,
                at: "seatmap.example".into(),
                why,
                tier: Tier::Work,
                count: 1,
            },
            settled: false,
        })
    };
    let talk = Conversation {
        lines: vec![line(7, Why::Fenced), line(8, Why::Order)],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let links = view.read_with(cx, |view, _| view.ilo_links.borrow().clone());
    assert_eq!(links.keys().copied().collect::<Vec<_>>(), [7], "{links:?}");
    host.take();
    cx.simulate_click(links[&7].center(), Modifiers::none());
    assert_eq!(host.take(), ["off limits"]);
}

/// A step up on a Stopped line takes no press until the line has stood a moment, as an agent
/// decides when it comes; once it has, one press raises the task's access, with no dialog.
#[gpui::test]
fn a_step_up_takes_no_press_until_its_line_has_stood(cx: &mut TestAppContext) {
    let (_shell, host, view, cx) = stopped_at_order(cx);
    let raise = (7, IloButton::Raise(Tier::Full));
    let (at, _) = ilo_button(&view, raise, cx).expect("Give Full access wasn't drawn");
    assert!(
        ilo_button(&view, (7, IloButton::TakeOver), cx).is_some(),
        "Take Over wasn't drawn"
    );
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    assert!(
        !host.take().iter().any(|line| line.contains("SetTier")),
        "a press as the line came raised the task"
    );
    // As if it had stood there since longer ago than the wait.
    let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(5);
    view.update(cx, |view, _| {
        view.ilo_drawn.borrow_mut().insert(raise, (at, long_ago));
    });
    cx.simulate_click(at.center(), Modifiers::none());
    assert_eq!(
        host.take()
            .into_iter()
            .filter(|line| line.contains("SetTier"))
            .collect::<Vec<_>>(),
        vec!["task task-1 SetTier(Full)".to_string()],
        "one press on a line that had stood didn't raise it to Full access"
    );
}

/// No key presses an ilo line's button, wherever the keyboard is in the conversation: it reaches
/// one only through the user's Focus command (#3955).
#[gpui::test]
fn no_key_presses_an_ilo_lines_button(cx: &mut TestAppContext) {
    let (_shell, host, view, cx) = stopped_at_order(cx);
    let raise = (7, IloButton::Raise(Tier::Full));
    let (at, _) = ilo_button(&view, raise, cx).expect("Give Full access wasn't drawn");
    let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(5);
    view.update(cx, |view, _| {
        view.ilo_drawn.borrow_mut().insert(raise, (at, long_ago));
    });
    host.take();
    for keys in ["enter", "space", "tab enter", "tab tab space", "cmd-enter"] {
        cx.simulate_keystrokes(keys);
    }
    assert!(
        !host
            .take()
            .iter()
            .any(|line| line.contains("SetTier") || line.contains("TakeOver")),
        "a key pressed an ilo line's button"
    );
}

/// A task that is done and takes no reply (#4105): ilo's line stands where its reply field was,
/// so nothing typed is sent to it, and Start Again opens the command bar's ask with what the task
/// was first asked, which sends nothing. Without the line its field is back.
#[gpui::test]
fn a_task_that_takes_no_reply_offers_start_again_in_its_fields_place(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    assert!(view.read_with(cx, |view, _| view.takes_reply()));
    let talk = Conversation {
        lines: vec![
            Line::Said("Two seats at 9:05, car 4.".into()),
            Line::Ilo(IloLine {
                key: usize::MAX,
                kind: IloKind::KeepsNothing,
                settled: false,
            }),
        ],
        stoppable: false,
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(
        !view.read_with(cx, |view, _| view.takes_reply()),
        "the reply field stayed beside ilo's line"
    );
    let start = (usize::MAX, IloButton::StartAgain);
    let (at, _) = ilo_button(&view, start, cx).expect("Start Again wasn't drawn");
    let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(5);
    view.update(cx, |view, _| {
        view.ilo_drawn.borrow_mut().insert(start, (at, long_ago));
    });
    host.take();
    cx.simulate_keystrokes("a n d enter");
    cx.simulate_click(at.center(), Modifiers::none());
    cx.run_until_parked();
    let did = host.take();
    assert!(
        did.contains(&"ask again a flight to Lisbon next Friday".to_string()),
        "Start Again didn't open the ask with the task's first request: {did:?}"
    );
    assert!(
        !did.iter().any(|line| line.contains("Reply")),
        "something was sent to the task: {did:?}"
    );

    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |view, _| view.takes_reply()));
}

/// Task-1's conversation, done, with one of ilo's lines of `kind` at its end.
fn ended(kind: IloKind) -> Conversation {
    Conversation {
        lines: vec![
            Line::Said("Two seats at 9:05, car 4.".into()),
            Line::Ilo(IloLine {
                key: 7,
                kind,
                settled: false,
            }),
        ],
        stoppable: false,
        ..conversation(0)
    }
}

/// Where the words of ilo's line 7 were drawn and how they were laid out, and where the
/// conversation was, once it has drawn again.
fn ilo_words(
    view: &Entity<super::TalkView>,
    cx: &mut VisualTestContext,
) -> (
    gpui::Bounds<gpui::Pixels>,
    gpui::TextLayout,
    gpui::Bounds<gpui::Pixels>,
) {
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let (words, laid) =
            (view.ilo_words_drawn.borrow().get(&7).cloned()).expect("ilo's line wasn't drawn");
        (words, laid, view.drawn_at.get().unwrap())
    })
}

/// Narrows the window until the conversation is no wider than the panel beside a page, the
/// narrowest it gets.
fn narrowest(view: &Entity<super::TalkView>, cx: &mut VisualTestContext) {
    let (_, _, area) = ilo_words(view, cx);
    let size = cx.update(|window, _| window.viewport_size());
    let narrow = px(BESIDE) - px(1.);
    cx.simulate_resize(gpui::size(
        size.width - (area.size.width - narrow),
        size.height,
    ));
    let (_, _, area) = ilo_words(view, cx);
    assert!(
        area.size.width <= px(BESIDE),
        "the conversation is {:?} wide, wider than the panel beside a page",
        area.size.width
    );
}

/// One of ilo's lines wraps and is never cut (#4105): in a conversation as narrow as the panel
/// beside a page, the line that says a conversation is kept lays its whole sentence out on more
/// lines, inside the column.
#[gpui::test]
fn ilos_line_wraps_its_whole_sentence_in_the_narrowest_conversation(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let show = |kind: IloKind, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(ended(kind)), cx)
        });
    };
    // Words that fit are one line, as tall as a trace's.
    show(IloKind::KeepsNothing, cx);
    let (short, _, _) = ilo_words(&view, cx);
    assert_eq!(f32::from(short.size.height), TRACE_LINE);

    show(IloKind::KeptHere, cx);
    narrowest(&view, cx);
    let (wrapped, laid, area) = ilo_words(&view, cx);
    let whole = cx.update(|_, cx| {
        let system = WindowTextSystem::new(cx.text_system().clone());
        text_width(&laid.text().into(), text::BODY, FontWeight::NORMAL, &system)
    });
    assert!(
        laid.text().ends_with("so you can reply."),
        "not the whole sentence: {}",
        laid.text()
    );
    assert!(
        wrapped.left() >= area.left() + px(INSET) && wrapped.right() <= area.right() - px(INSET),
        "ilo's words at {wrapped:?} leave the column of {area:?}"
    );
    let lines =
        ((f32::from(wrapped.size.height) - (TRACE_LINE - ILO_TEXT_LINE)) / ILO_TEXT_LINE).round();
    assert!(lines >= 2., "ilo's words stayed on one line: {wrapped:?}");
    assert!(
        lines * f32::from(wrapped.size.width) >= whole,
        "{lines} lines of {:?} don't hold the sentence, {whole} long",
        wrapped.size.width
    );
}

/// An ilo line has one left edge, its mark's (#4105): in the narrowest conversation the line its
/// words wrap to starts under the mark, and its buttons start there too.
#[gpui::test]
fn ilos_wrapped_line_and_its_buttons_start_under_its_mark(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let stopped = IloKind::Stopped {
        tab: 30,
        at: "Place the order for both of the seats".into(),
        why: Why::Order,
        tier: Tier::Work,
        count: 1,
    };
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(ended(stopped)), cx)
    });
    narrowest(&view, cx);
    let (words, laid, _) = ilo_words(&view, cx);
    let text = laid.text();
    assert!(
        text.starts_with(super::words::MARK),
        "ilo's mark isn't the first of its words: {text}"
    );
    let mark = laid.position_for_index(0).unwrap();
    assert_eq!(
        mark.x,
        words.left(),
        "the mark isn't at its words' left edge"
    );
    let line = laid.line_height();
    // The character at the mark's left edge, a line down: the first of the wrapped line.
    let wrapped = laid
        .index_for_position(point(mark.x, mark.y + line * 1.5))
        .expect("ilo's words didn't wrap under its mark");
    assert!(wrapped > super::words::MARK.len(), "{wrapped} of {text}");
    // The tests' text system sets every character one advance wide, so the wrapped line's second
    // character is as far from its start as the mark's second is from the mark's.
    let step = |from: usize| from + text[from..].chars().next().unwrap().len_utf8();
    let (in_mark, in_wrapped) = (
        laid.position_for_index(step(0)).unwrap(),
        laid.position_for_index(step(wrapped)).unwrap(),
    );
    assert_eq!(
        in_wrapped.y,
        mark.y + line,
        "not on the line under the mark"
    );
    assert_eq!(
        in_wrapped.x, in_mark.x,
        "the wrapped line doesn't start at the mark's left edge"
    );
    let (button, _) = ilo_button(&view, (7, IloButton::TakeOver), cx).expect("no Take Over");
    assert_eq!(button.left(), mark.x, "its buttons don't start under ilo");
}

/// Whether the level's chip under the reply field is drawn for task-1's conversation, done, with
/// ilo's line of `kind` at its end, beside the page it watched.
fn level_chip_with(kind: IloKind, cx: &mut TestAppContext) -> bool {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_talk_beside(true, cx);
        shell.set_conversation(Some(ended(kind)), cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    view.read_with(cx, |view, _| view.level_drawn.get().is_some())
}

/// A task that takes a reply keeps its level under the field, ended and picked up again too.
#[gpui::test]
fn a_task_that_takes_a_reply_keeps_its_level_under_the_field(cx: &mut TestAppContext) {
    assert!(level_chip_with(IloKind::KeptHere, cx));
}

/// With ilo's line and Start Again in the field's place there's no level to set, so the row under
/// the field goes too (#4105): for a conversation the assistant no longer has,
#[gpui::test]
fn a_task_whose_conversation_is_gone_has_no_level_row(cx: &mut TestAppContext) {
    assert!(!level_chip_with(IloKind::Gone, cx));
}

/// for one that ended while Settings' switch was off,
#[gpui::test]
fn a_task_that_wasnt_kept_has_no_level_row(cx: &mut TestAppContext) {
    assert!(!level_chip_with(IloKind::NotKept, cx));
}

/// and for an assistant that keeps no finished task.
#[gpui::test]
fn a_task_of_an_assistant_that_keeps_nothing_has_no_level_row(cx: &mut TestAppContext) {
    assert!(!level_chip_with(IloKind::KeepsNothing, cx));
}

/// Task-1's conversation as it works with `helpers` helpers (#4102), each reading its shop's
/// page, and its own step under way after them.
fn with_helpers(helpers: usize) -> Conversation {
    let mut talk = conversation(1);
    let items = (0..helpers)
        .map(|at| TraceItem::Helper(step(10 + at, "Reading shop.example", true)))
        .chain([TraceItem::Step(step(0, "Reading reviews.example", true))]);
    talk.lines = vec![Line::Trace(Trace {
        key: 0,
        running: true,
        took: None,
        items: items.collect(),
    })];
    talk
}

/// The one trace of `talk`.
fn its_trace(talk: &mut Conversation) -> &mut Trace {
    match &mut talk.lines[0] {
        Line::Trace(trace) => trace,
        line => panic!("{line:?}"),
    }
}

/// A task at work with helpers says how many on its Working line, one in its own form, in place
/// of its steps, and still once its turn is over, as its helpers' lines stay; without one, the
/// line counts its steps as it did.
#[test]
fn a_working_line_counts_the_tasks_helpers() {
    let summary = |talk: &mut Conversation| {
        let (title, counted) = its_trace(talk).summary();
        format!("{title} · {counted}")
    };
    assert_eq!(summary(&mut with_helpers(4)), "Working · 4 helpers");
    assert_eq!(summary(&mut with_helpers(1)), "Working · 1 helper");
    assert_eq!(summary(&mut with_helpers(0)), "Working · 1 step");

    // One of the four waits on the user and one has finished: each still has its line, so each
    // still counts.
    let mut talk = with_helpers(4);
    its_trace(&mut talk).items[1] = TraceItem::Helper(step(11, "Read shop.example", false));
    its_trace(&mut talk).items[2] = TraceItem::Helper(step(12, "Handed you shop.example", false));
    assert_eq!(summary(&mut talk), "Working · 4 helpers");

    let over = its_trace(&mut talk);
    (over.running, over.took) = (false, Some(41));
    assert_eq!(summary(&mut talk), "Worked for 41 s · 4 helpers");
}

/// On Pages, a task on one page rings the page it's on, with the step under way. One that works
/// on several rings every page that has a step of its own, each saying its own and never
/// another's, but not the page the user holds, nor one with no step yet (#4102).
#[test]
fn every_page_a_task_works_on_is_ringed_with_its_own_step() {
    use super::picture_work;
    use gpui::SharedString;
    let running: Option<SharedString> = Some("Clicking “Tickets”".into());
    assert_eq!(
        picture_work(1, Some(1), running.clone(), &[], false),
        (true, running.clone())
    );
    assert!(!picture_work(2, Some(1), running.clone(), &[], false).0);

    let pages: Vec<(u64, SharedString)> = vec![
        (1, "Reading tickets.example".into()),
        (2, "Reading stalls.example".into()),
        (3, "Handing you boxoffice.example".into()),
    ];
    for (tab, step) in &pages[..2] {
        let work = picture_work(*tab, Some(1), running.clone(), &pages, false);
        assert_eq!(work, (true, Some(step.clone())), "page {tab}");
    }
    assert!(
        !picture_work(3, Some(1), running.clone(), &pages, true).0,
        "the held page"
    );
    assert_eq!(
        picture_work(4, Some(1), running, &pages, false),
        (false, None)
    );
}

/// Pages' grid holds as many pictures across as fit at their full size, up to the most a row has.
/// Where only one fits, as beside a page, three pages or more go two across, each half the room,
/// so no one scrolls for the third; one page or two stay whole.
#[test]
fn three_pages_or_more_are_never_fewer_than_two_across() {
    use super::grid_shape;
    use crate::tokens::shell::conversation::{PICTURE_GAP, PICTURE_WIDTH, PICTURES_COLUMNS};

    // Room for one picture and most of a second.
    let one = PICTURE_WIDTH * 1.75;
    for pages in [1, 2] {
        assert_eq!(grid_shape(one, pages), (1, PICTURE_WIDTH), "{pages} pages");
    }
    for pages in [3, 4, 9] {
        let (across, wide) = grid_shape(one, pages);
        assert_eq!(across, 2, "{pages} pages");
        assert!(wide < PICTURE_WIDTH, "{wide}");
        let (row, spare) = (2. * wide + PICTURE_GAP, one - (2. * wide + PICTURE_GAP));
        assert!(
            row <= one && spare < 2.,
            "the two don't fill the room: {row} of {one}"
        );
    }
    // With room for more, each is whole however many there are.
    let two = 2. * PICTURE_WIDTH + PICTURE_GAP;
    assert_eq!(grid_shape(two, 4), (2, PICTURE_WIDTH));
    let wide = (PICTURES_COLUMNS + 1.) * (PICTURE_WIDTH + PICTURE_GAP);
    for pages in [1, 4] {
        assert_eq!(
            grid_shape(wide, pages),
            (PICTURES_COLUMNS as usize, PICTURE_WIDTH)
        );
    }
}

/// Opened, as it is while it works, the Working line lists a line for each helper with the task's
/// own steps, and folds them away with a press, as any trace's.
#[gpui::test]
fn a_working_line_opens_to_a_line_for_each_helper(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(with_helpers(4)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };
    let listed = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            view.is_open(Fold::Trace(0), true)
                .then(|| view.heights.borrow()[&Fold::Trace(0)])
        })
    };
    draw(cx);
    assert_eq!(
        listed(cx),
        Some(5. * TRACE_LINE),
        "four helpers' lines and the task's own step"
    );

    // A fifth line as a helper comes, and none goes as one finishes.
    let mut talk = with_helpers(5);
    its_trace(&mut talk).items[0] = TraceItem::Helper(step(10, "Read shop.example", false));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    draw(cx);
    assert_eq!(listed(cx), Some(6. * TRACE_LINE));

    view.update(cx, |view, cx| view.toggle(Fold::Trace(0), true, cx));
    draw(cx);
    assert_eq!(listed(cx), None, "folded, it still listed them");
}

/// Beside its page, Take Over stands once on the lines, on the task's own step under way, which
/// is on the page it takes: a helper's line under way has none, as its page is another.
#[gpui::test]
fn a_helpers_line_has_no_take_over(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_talk_beside(true, cx);
        shell.set_conversation(Some(with_helpers(4)), cx);
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let chips = view.read_with(cx, |view, _| view.take_over_drawn.borrow().clone());
    assert_eq!(chips.len(), 1, "{chips:?}");
    host.take();
    cx.simulate_click(chips[0].center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 TakeOver(30)"]);

    // With only helpers under way, no line is the page's.
    let mut talk = with_helpers(4);
    its_trace(&mut talk).items.pop();
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let chips = view.read_with(cx, |view, _| view.take_over_drawn.borrow().len());
    assert_eq!(chips, 0);
}

/// A helper's line under way shimmers as a step's does, so the view asks for frames while one
/// works, though the task's own step has finished; under Reduce Motion, which the view asks
/// `motion::still`, it holds; and a helper waiting on the user, or done, moves nothing.
#[gpui::test]
fn a_helpers_line_under_way_shimmers_unless_motion_is_reduced(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let mut talk = with_helpers(4);
    its_trace(&mut talk).items[4] = TraceItem::Step(step(0, "Read reviews.example", false));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(talk.clone()), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let moving = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let still = crate::motion::still(cx);
            view.read(cx).moving(still)
        })
    };
    // `shown` reduces motion.
    assert!(!moving(cx), "a helper's line moved under Reduce Motion");
    cx.update(|_, cx| cx.set_reduce_motion(false));
    assert!(moving(cx), "a helper under way didn't shimmer");

    // Three have finished and the fourth waits for the user to sign in.
    for at in 0..3 {
        its_trace(&mut talk).items[at] =
            TraceItem::Helper(step(10 + at, "Read shop.example", false));
    }
    its_trace(&mut talk).items[3] = TraceItem::Helper(step(13, "Handed you shop.example", false));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    assert!(!moving(cx), "nothing is under way");
    cx.update(|_, cx| cx.set_reduce_motion(true));
}
