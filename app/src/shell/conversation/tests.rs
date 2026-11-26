//! A task's card and its conversation in the page area, on a host that records what the shell asks
//! of the core.

use std::rc::Rc;

use gpui::{
    Entity, Focusable, FontWeight, Modifiers, TestAppContext, VisualTestContext, WindowTextSystem,
    point, px,
};

use super::Fold;

use crate::agents::task::ilo::{HelpersLimit, IloButton, IloKind, IloLine, LeftOut, Reach};
use crate::agents::task::{
    Card, Conversation, Dot, Line, Opens, Trace, TraceItem, TraceStep, Word,
};
use crate::agents::tier::{Tier, Why};
use crate::shell::picture::PagePicture;
use crate::shell::{Command, Folders, Page, Shell, Trouble, recorder::Recorder};
use crate::text_input::TextInput;
use crate::tokens::shell::conversation::ilo::TEXT_LINE as ILO_TEXT_LINE;
use crate::tokens::shell::conversation::{INSET, SITE, SITE_GAP, TRACE_LINE};
use crate::tokens::shell::task_area::BESIDE;
use crate::tokens::text;
use crate::tooltip::text_width;

pub(super) fn card() -> Card {
    Card {
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Flight to Lisbon".into(),
        unnamed: false,
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_tab: None,
        line: None,
        ended_at: None,
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
        visited: Vec::new(),
        ended_time: Default::default(),
        finding: Default::default(),
        told: Default::default(),
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
pub(super) fn conversation(steps: usize) -> Conversation {
    let items = (0..steps)
        .map(|key| TraceItem::Step(step(key, "Opened skyscanner.net", key + 1 == steps)))
        .collect();
    Conversation {
        agent: "task-1".into(),
        name: "Claude".into(),
        title: "Flight to Lisbon".into(),
        about: "Claude · Haiku · Working".into(),
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
        live: Default::default(),
        open: None,
        answers: Vec::new(),
        writing: false,
        tier: Default::default(),
        only_added: false,
        folder: None,
    }
}

/// The request the conversation's view holds out for `tab`'s picture, as it asked for it. Not the
/// app's counter less one: the tests run on threads of one process, which all take from it.
fn picture_request(shell: &Entity<Shell>, tab: u64, cx: &mut VisualTestContext) -> u64 {
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.read_with(cx, |view, _| {
        let held = view.pictures.iter().find(|(each, ..)| *each == tab);
        held.map(|(_, request, _)| *request)
            .expect("the view asked for no picture of the tab")
    })
}

/// The user's Space 1 with tabs 10 and 11, 10 shown, and task-1's Space 3 with tab 30, whose card
/// is at the top of Space 1's Today; a click on the card shows its conversation. Without motion,
/// so the area is in place at once.
pub(super) fn shown(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
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
    let when = crate::needs_you::Notify::Away;
    cx.update(|_, cx| crate::needs_you::follow(&[waits], when, cx));
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

/// A task's pages stand in a rail beside its conversation, one view with no switch: a tile's click
/// opens its page, focusing no page; Esc puts the page back in its tile with the keyboard back in
/// the reply field, and Esc again closes the task.
#[gpui::test]
fn two_pages_stand_in_the_rail_beside_the_conversation(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "seatmap.example".into()),
    ];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let tiles: Vec<u64> = view.read_with(cx, |view, _| {
        view.drawn.borrow().iter().map(|(tab, _)| *tab).collect()
    });
    assert_eq!(tiles, [30, 31]);
    let (_, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    let lines = host.take();
    assert!(lines.contains(&"select 30".to_string()), "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.starts_with("focus page")),
        "a tile's click focused its page: {lines:?}"
    );
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
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(
        cx.update(|window, _| reply.is_focused(window)),
        "the reply field hasn't the keys back after Esc"
    );
    shell.update(cx, |shell, cx| shell.task_step_back(cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_shown()), None);
}

/// One view at every width: wide, the tiles stand one under another at the area's right edge, past
/// the conversation's column; narrow, they fall to one row under the header. Either way the
/// conversation's lines and its reply field show with them.
#[gpui::test]
fn one_view_at_every_width(cx: &mut TestAppContext) {
    use crate::tokens::shell::task::RAIL_BREAKPOINT;
    use gpui::size;
    let (shell, _, cx) = shown(cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "seatmap.example".into()),
        (32, "stalls.example".into()),
    ];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |wide: f32, cx: &mut VisualTestContext| {
        cx.simulate_resize(size(px(wide), px(900.)));
        // The first frame learns the area's width; the second lays the rail out by it.
        for _ in 0..2 {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        let tiles = view.read_with(cx, |view, _| view.drawn.borrow().clone());
        let area = view.read_with(cx, |view, _| view.drawn_at.get()).unwrap();
        let reply = view.read_with(cx, |view, _| view.reply_drawn.get());
        (tiles, area, reply)
    };
    let (tiles, area, reply) = draw(RAIL_BREAKPOINT + 600., cx);
    assert_eq!(tiles.len(), 3, "{tiles:?}");
    let reply = reply.expect("no reply field beside the rail");
    for pair in tiles.windows(2) {
        let (above, below) = (pair[0].1, pair[1].1);
        assert_eq!(above.origin.x, below.origin.x, "{tiles:?}");
        assert!(below.origin.y > above.origin.y, "{tiles:?}");
    }
    assert!(
        tiles[0].1.origin.x >= reply.right(),
        "the rail is over the column"
    );
    assert!(
        area.right() - tiles[0].1.right() < px(80.),
        "the rail isn't at the right edge"
    );

    let (tiles, _, reply) = draw(RAIL_BREAKPOINT - 100., cx);
    assert_eq!(tiles.len(), 3, "{tiles:?}");
    let reply = reply.expect("no reply field under the row");
    for pair in tiles.windows(2) {
        let (before, after) = (pair[0].1, pair[1].1);
        assert_eq!(before.origin.y, after.origin.y, "{tiles:?}");
        assert!(after.origin.x > before.origin.x, "{tiles:?}");
    }
    assert!(
        tiles[0].1.bottom() < reply.top(),
        "the row isn't over the reply field"
    );
}

/// While its task works every page says Working, where each reads `page_mark`: the rail's tile and
/// the card's helper lines. A page that has
/// finished loading, or that the task isn't in, is still Working: Done comes only with the task.
#[gpui::test]
fn a_working_tasks_pages_all_say_working_where_each_is_read(cx: &mut TestAppContext) {
    let mut working = card();
    working.pages = vec![30, 31];
    working.page_steps = vec![(31, "Reading".into())];
    let (shell, _, cx) = shown_with(working, cx);
    cx.simulate_a11y_activation();
    let mut talk = conversation(1);
    talk.tabs = vec![(30, "tickets.example".into()), (31, "seats.example".into())];
    shell.update(cx, |shell, cx| {
        let folders = Folders::default();
        shell.set_tabs(3, Vec::new(), vec![30, 31], Vec::new(), folders, cx);
        for (tab, url) in [
            (30, "https://tickets.example/"),
            (31, "https://seats.example/"),
        ] {
            let page = Page {
                url: url.into(),
                loading: false,
                ..Page::default()
            };
            shell.set_page(tab, page, cx);
        }
        shell.set_conversation(Some(talk), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let tiles = view.read_with(cx, |view, _| {
        [30, 31].map(|tab| view.said_logged.borrow().get(&tab).copied())
    });
    assert_eq!(tiles, [Some("Working"); 2], "the rail's tiles");
    let helpers = shell.read_with(cx, |shell, _| {
        let pages = shell.helper_pages(&"task-1".into(), vec![30, 31]);
        pages.iter().map(|page| page.word).collect::<Vec<_>>()
    });
    assert_eq!(helpers, ["Working"; 2], "the card's helper lines");
}

/// A tile whose word turns to Needs you is brought into sight once, where the rail holds more
/// than fit, its tiles in their order; one the user scrolls away again stays away. Its page opens
/// by itself and takes no keyboard.
#[gpui::test]
fn a_tile_turning_to_needs_you_comes_into_sight_once(cx: &mut TestAppContext) {
    use crate::agents::Status;
    use crate::tokens::shell::task::RAIL_BREAKPOINT;
    use gpui::{point, size};
    let waiting = Card {
        dot: Dot::Waiting,
        word: Word::NeedsYou,
        ..card()
    };
    let (shell, host, cx) = shown_with(waiting, cx);
    let mut talk = conversation(1);
    talk.tabs = (30..36)
        .map(|tab| (tab, "tickets.example".into()))
        .collect();
    shell.update(cx, |shell, cx| {
        let tabs = (30..36).collect();
        shell.set_tabs(3, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        shell.set_conversation(Some(talk), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    cx.simulate_resize(size(px(RAIL_BREAKPOINT - 100.), px(900.)));
    let draw = |cx: &mut VisualTestContext| {
        for _ in 0..2 {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        view.read_with(cx, |view, _| view.rail_scroll.offset().x)
    };
    assert_eq!(draw(cx), px(0.), "the row started scrolled");
    let tiles = view.read_with(cx, |view, _| view.drawn.borrow().clone());
    let order: Vec<u64> = tiles.iter().map(|(tab, _)| *tab).collect();
    assert_eq!(order, [30, 31, 32, 33, 34, 35]);

    host.take();
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(35, "Sign in".into())],
        taken: Vec::new(),
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    assert!(
        draw(cx) < px(0.),
        "the tile that needs the user stayed out of sight"
    );
    let tiles = view.read_with(cx, |view, _| view.drawn.borrow().clone());
    let after: Vec<u64> = tiles.iter().map(|(tab, _)| *tab).collect();
    assert_eq!(after, order, "the tiles changed their order");
    // With no page in the area, the page that needs the user opens as itself (i).
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page().map(|(_, tab)| tab)),
        Some(35),
        "the page that needs the user didn't open"
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call == "focus page"),
        "the page ilo opened took the keyboard: {calls:?}"
    );

    // The user scrolls back to the start: the tile isn't brought back again.
    view.update(cx, |view, _| {
        view.rail_scroll.set_offset(point(px(0.), px(0.)))
    });
    assert_eq!(
        draw(cx),
        px(0.),
        "the rail scrolled to the tile a second time"
    );
}

/// A page that comes to need the user while the page they left goes back into its picture opens
/// only once that one is in its thumbnail, never over it on its way.
#[gpui::test]
fn a_needs_you_page_opens_once_the_page_left_is_back(cx: &mut TestAppContext) {
    use crate::agents::Status;
    let waiting = Card {
        dot: Dot::Waiting,
        word: Word::NeedsYou,
        ..card()
    };
    let (shell, host, cx) = shown_with(waiting, cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "tickets.example".into()),
    ];
    shell.update(cx, |shell, cx| {
        let tabs = vec![30, 31];
        shell.set_tabs(3, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        shell.set_conversation(Some(talk), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        for _ in 0..2 {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
    };
    draw(cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    shell.update(cx, |shell, cx| {
        assert!(
            shell.watch_row(30, false, cx),
            "the user's page didn't open"
        );
        shell.hold_task_page_going_back();
        cx.notify();
    });
    draw(cx);
    host.take();

    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(31, "Sign in".into())],
        taken: Vec::new(),
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    draw(cx);
    assert!(
        shell.read_with(cx, |shell, _| shell.task_page_going_back()),
        "the page the user left stopped going back"
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call == "select 31"),
        "the page that needs the user opened over the one going back: {calls:?}"
    );

    // The page left is in its thumbnail at the zoom's next step: then the other opens.
    cx.update(|_, cx| cx.set_reduce_motion(true));
    draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page().map(|(_, tab)| tab)),
        Some(31),
        "the page that needs the user didn't open"
    );
    let calls = host.take();
    assert!(calls.iter().any(|call| call == "select 31"), "{calls:?}");
    assert!(
        !calls.iter().any(|call| call == "focus page"),
        "the page ilo opened took the keyboard: {calls:?}"
    );
}

/// A page ilo would open by itself over the reply field doesn't open while the user types there:
/// the field keeps the keyboard and its text, and the tile that needs them is in sight.
#[gpui::test]
fn a_needs_you_page_never_covers_the_reply_field_in_use(cx: &mut TestAppContext) {
    let reply = needs_you_beside_typing(
        |reply, window, cx| reply.set_text("Two seats".into(), 9, window, cx),
        cx,
    );
    assert_eq!(reply, "Two seats");
}

/// A composition under way is typing too, before the input method commits a word of it: a page
/// opening then would take the keyboard mid-word. The field holds the marked text in its text.
#[gpui::test]
fn a_needs_you_page_never_covers_a_composition_under_way(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let reply = needs_you_beside_typing(
        |reply, window, cx| reply.replace_and_mark_text_in_range(None, "か", None, window, cx),
        cx,
    );
    assert_eq!(reply, "か", "the composition was lost");
}

/// Types into the focused reply field with `type_in`, turns a tile to Needs you, checks ilo
/// opened no page over the field and left it the keyboard, and gives back the field's text.
fn needs_you_beside_typing(
    type_in: impl FnOnce(&mut TextInput, &mut gpui::Window, &mut gpui::Context<TextInput>),
    cx: &mut TestAppContext,
) -> String {
    use crate::agents::Status;
    let waiting = Card {
        dot: Dot::Waiting,
        word: Word::NeedsYou,
        ..card()
    };
    let (shell, host, cx) = shown_with(waiting, cx);
    let mut talk = conversation(1);
    talk.tabs = vec![
        (30, "tickets.example".into()),
        (31, "tickets.example".into()),
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
        shell.set_conversation(Some(talk), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let reply = view.read_with(cx, |view, _| view.reply.clone());
    cx.update(|window, cx| {
        reply.update(cx, |reply, cx| type_in(reply, window, cx));
        let handle = reply.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    host.take();
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(31, "Sign in".into())],
        taken: Vec::new(),
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page()),
        None,
        "the page covered the reply field the user types in"
    );
    let calls = host.take();
    assert!(!calls.iter().any(|call| call == "focus page"), "{calls:?}");
    let handle = reply.read_with(cx, |reply, cx| reply.focus_handle(cx));
    assert!(
        cx.update(|window, _| handle.is_focused(window)),
        "the reply field lost the keyboard"
    );
    reply.read_with(cx, |reply, _| reply.text().to_string())
}

/// A picture's click zooms its page out of it, at the picture's place and with its image; what
/// the task does meanwhile isn't pictured under the page, but is once the page is back.
#[gpui::test]
fn a_picture_shows_its_page(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let request = picture_request(&shell, 30, cx);
    let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
    shell.update(cx, |shell, cx| shell.page_pictured(request, picture, cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    // One page has a rail of one.
    view.update(cx, |_, cx| cx.notify());
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
                gpui::Hsla::from(shell.palette.gray(FORM_PLACEHOLDER)),
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

/// Task-1's conversation with ilo's line that it stopped Claude at Place order, under Work on
/// pages, drawn, and its view.
pub(super) fn stopped_at_order(
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
                doing: "clicking “Place order”".into(),
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
            "ilo stopped Claude from clicking “Place order”\n\nPlacing an order isn't part of Work on pages.\n\n\
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
        doing: "".into(),
        why,
        tier: Tier::Work,
        count: 1,
    };
    let said = |kind: &IloKind| {
        let (own, name) = super::words::head(kind, "Claude");
        super::words::with_name(kind, &own, &name.unwrap()).0
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
        doing: "clicking “Place order”".into(),
        why: Why::Order,
        tier: Tier::Work,
        count: 1,
    };
    // B1: the refused step as it was under way, its control's name in its own quotes alone.
    assert_eq!(
        super::words::head(&order, "Claude"),
        (
            "stopped Claude from clicking \u{201c}Place order\u{201d}".into(),
            None
        )
    );
    // With no step of its own traced, it names none.
    let untraced = IloKind::Stopped {
        tab: 30,
        at: "Place order".into(),
        doing: "".into(),
        why: Why::Order,
        tier: Tier::Work,
        count: 1,
    };
    assert_eq!(
        super::words::head(&untraced, "Claude"),
        ("stopped one of Claude's steps".into(), None)
    );
    // O1: a step on one of several pages names its site, in the middle of ilo's words.
    let yours = IloKind::Yours {
        tab: 30,
        site: Some("boxoffice.example".into()),
    };
    assert_eq!(said(&yours), "handed this step on boxoffice.example to you");
    let (own, name) = super::words::head(&yours, "Claude");
    let (line, at) = super::words::with_name(&yours, &own, &name.unwrap());
    assert_eq!(&line[at], "boxoffice.example");
}

/// The line of a task the user stopped says the pages it held then by their count alone, and with
/// none, no count (Z17). It names nothing, so no page's or site's words stand in it, and it has
/// nothing to press.
#[gpui::test]
fn the_users_stop_says_its_pages_by_their_count_alone(cx: &mut TestAppContext) {
    let said = |pages| super::words::head(&IloKind::YouStopped { pages }, "Claude");
    assert_eq!(said(0), ("· You stopped it".into(), None));
    assert_eq!(said(1), ("· You stopped it · 1 page".into(), None));
    assert_eq!(said(3), ("· You stopped it · 3 pages".into(), None));

    let (shell, _host, cx) = shown(cx);
    let talk = Conversation {
        lines: vec![Line::Ilo(IloLine {
            key: 7,
            kind: IloKind::YouStopped { pages: 3 },
            settled: true,
        })],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let (_, laid, _) = ilo_words(&view, cx);
    assert_eq!(laid.text(), "ilo · You stopped it · 3 pages");
    assert!(
        view.read_with(cx, |view, _| view.ilo_drawn.borrow().is_empty()),
        "the line of the user's stop drew a button"
    );
}

/// The stopped line of an outside agent's task offers its pages back until they're handed back:
/// `Hand Back`, its one button, which gives the agent pages again and so takes no press until the
/// line has stood a moment. Once it has, one press hands them back, and no key does.
#[gpui::test]
fn the_stopped_lines_hand_back_takes_no_press_until_it_has_stood(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let talk = Conversation {
        lines: vec![Line::Ilo(IloLine {
            key: 7,
            kind: IloKind::YouStopped { pages: 2 },
            settled: false,
        })],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let back = (7, IloButton::PagesBack);
    let (at, _) = ilo_button(&view, back, cx).expect("Hand Back wasn't drawn");
    assert_eq!(super::words::button(IloButton::PagesBack), "Hand Back");
    assert_eq!(
        view.read_with(cx, |view, _| view.ilo_drawn.borrow().len()),
        1,
        "the stopped line has one button"
    );
    host.take();
    cx.simulate_click(at.center(), Modifiers::none());
    assert!(
        !host.take().iter().any(|line| line.contains("PagesBack")),
        "a press as the line came handed the pages back"
    );
    let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(5);
    view.update(cx, |view, _| {
        view.ilo_drawn.borrow_mut().insert(back, (at, long_ago));
    });
    cx.simulate_click(at.center(), Modifiers::none());
    assert_eq!(
        host.take()
            .into_iter()
            .filter(|line| line.contains("PagesBack"))
            .collect::<Vec<_>>(),
        vec!["task task-1 PagesBack".to_string()],
    );
    // Stood as long as it has, no key presses it.
    for keys in ["enter", "space", "tab enter", "cmd-enter"] {
        cx.simulate_keystrokes(keys);
    }
    assert!(
        !host.take().iter().any(|line| line.contains("PagesBack")),
        "a key handed the pages back"
    );
}

/// The Focus command's keys reach the stopped line's Hand Back as they do any ilo line's button,
/// under the same wait as its click: ↩ presses it only once the keys have been there a moment
/// and the button has stood where it is.
#[gpui::test]
fn the_focus_keys_press_hand_back_only_once_it_has_stood(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let talk = Conversation {
        lines: vec![Line::Ilo(IloLine {
            key: 7,
            kind: IloKind::YouStopped { pages: 2 },
            settled: false,
        })],
        ..conversation(0)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let pressed = |host: &Rc<Recorder>| -> Vec<String> {
        (host.take().into_iter())
            .filter(|line| line.contains("PagesBack"))
            .collect()
    };
    host.take();
    focus_command(&shell, "ui", cx);
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        pressed(&host),
        Vec::<String>::new(),
        "a key as the keys came"
    );
    // The keys have been there long enough, the button hasn't stood.
    let keys_waited = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.update(cx, |view, _| {
            let keys = view.ilo_keys.as_mut().expect("the keys left the line");
            keys.last -= crate::agents::tier::PROTECT * 2;
        });
    };
    keys_waited(&view, cx);
    view.update(cx, |view, _| {
        for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
            *since = std::time::Instant::now();
        }
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(
        pressed(&host),
        Vec::<String>::new(),
        "a key before it had stood"
    );
    keys_waited(&view, cx);
    view.update(cx, |view, _| {
        for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
            *since -= std::time::Duration::from_secs(5);
        }
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(pressed(&host), ["task task-1 PagesBack"]);
}

/// The lines of a wall ilo couldn't put up around a task's files, and of a run it couldn't end
/// whole (#4101), each whole: what ilo did, then why, with the folder's or the file's name quoted
/// as the user's own and nothing else of the line drawn as a name.
#[gpui::test]
fn a_wall_that_wasnt_put_up_says_what_ilo_did_and_why(_: &mut TestAppContext) {
    use super::words::{head, tail, with_name};
    let said = |kind: &IloKind| {
        let (own, name) = head(kind, "Claude");
        let own = format!("ilo {own}");
        let Some(name) = name else {
            assert_eq!(tail(kind, "Claude"), None);
            return (own, String::new());
        };
        let (mut said, at) = with_name(kind, &own, &name);
        let quoted = said[at].to_string();
        said.extend(tail(kind, "Claude"));
        (said, quoted)
    };
    let without = |left| IloKind::StartedWithout { left };
    let stopped = |left| IloKind::NotRestarted { left };
    let left = |reach| IloKind::LeftRunning { reach };
    let cases = [
        (
            without(LeftOut::Folder("Trip".into())),
            "ilo started this task without \u{201c}Trip\u{201d}: ilo couldn't limit Claude to \
             that folder",
            "\u{201c}Trip\u{201d}",
        ),
        (
            without(LeftOut::File("seats.pdf".into())),
            "ilo started this task without \u{201c}seats.pdf\u{201d}: ilo couldn't limit Claude \
             to that file",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            without(LeftOut::Files(3)),
            "ilo started this task without your 3 files: ilo couldn't limit Claude to them",
            "",
        ),
        (
            IloKind::KeptAt {
                at: Tier::Work,
                refused: Tier::Full,
                folder: "Trip".into(),
            },
            "ilo kept this task at Work on pages: ilo couldn't limit Claude to \
             \u{201c}Trip\u{201d} at Full access",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::SetBack {
                at: Tier::Work,
                refused: Tier::Full,
                folder: "Trip".into(),
            },
            "ilo set this task back to Work on pages: ilo couldn't limit Claude to \
             \u{201c}Trip\u{201d} at Full access",
            "\u{201c}Trip\u{201d}",
        ),
        (
            stopped(LeftOut::Folder("Trip".into())),
            "ilo stopped this task: ilo couldn't restart Claude without \u{201c}Trip\u{201d}",
            "\u{201c}Trip\u{201d}",
        ),
        (
            stopped(LeftOut::File("seats.pdf".into())),
            "ilo stopped this task: ilo couldn't restart Claude without \
             \u{201c}seats.pdf\u{201d}",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            stopped(LeftOut::Files(3)),
            "ilo stopped this task: ilo couldn't restart Claude without your 3 files",
            "",
        ),
        (
            IloKind::NotRaised {
                refused: Tier::Full,
            },
            "ilo stopped this task: ilo couldn't restart Claude at Full access",
            "",
        ),
        (
            left(Reach::Folder("Trip".into())),
            "ilo couldn't end everything Claude started: what's left can still reach \
             \u{201c}Trip\u{201d}. Restarting this computer ends it.",
            "\u{201c}Trip\u{201d}",
        ),
        (
            left(Reach::Files),
            "ilo couldn't end everything Claude started: what's left can still reach this \
             task's files. Restarting this computer ends it.",
            "",
        ),
        (
            left(Reach::Both("Trip".into())),
            "ilo couldn't end everything Claude started: what's left can still reach \
             \u{201c}Trip\u{201d} and this task's files. Restarting this computer ends it.",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::EndedLeft,
            "ilo ended what Claude had left running",
            "",
        ),
        (
            IloKind::Revoked {
                folder: "Trip".into(),
            },
            "ilo \u{b7} You revoked \u{201c}Trip\u{201d}",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::FolderGone {
                folder: "Trip".into(),
            },
            "ilo revoked \u{201c}Trip\u{201d}: ilo couldn't find it",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::NotCopied {
                left: LeftOut::File("seats.pdf".into()),
            },
            "ilo started this task without \u{201c}seats.pdf\u{201d}: ilo couldn't copy it",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            IloKind::NotCopied {
                left: LeftOut::Files(3),
            },
            "ilo started this task without 3 files: ilo couldn't copy them",
            "",
        ),
    ];
    for (kind, whole, name) in cases {
        assert_eq!(
            said(&kind),
            (whole.to_string(), name.to_string()),
            "{kind:?}"
        );
        assert_eq!(super::words::body(&kind, "Trip"), None);
        let line = IloLine {
            key: 0,
            kind,
            settled: false,
        };
        assert!(crate::agents::task::ilo::buttons(&line).is_empty());
    }
}

/// An ilo line's head is a sentence: where ilo is its subject the mark is its first word and a verb
/// of ilo's follows ("ilo stopped Claude at …"); where it isn't, a middle dot sets the mark off
/// and the sentence starts with a capital ("ilo · Claude needs this tab back").
#[gpui::test]
fn an_ilo_lines_head_reads_as_a_sentence_after_the_mark(_: &mut TestAppContext) {
    use super::words::head;
    use crate::agents::task::ilo::SignedInWith;
    // ilo's verbs. A new head that starts with another word fails here, to be read as a sentence.
    const VERBS: [&str; 10] = [
        "stopped", "signed", "handed", "resumed", "started", "kept", "set", "couldn't", "ended",
        "revoked",
    ];
    let every = [
        IloKind::Stopped {
            tab: 1,
            at: "Place order".into(),
            doing: "clicking “Place order”".into(),
            why: Why::Order,
            tier: Tier::Work,
            count: 1,
        },
        IloKind::Stopped {
            tab: 1,
            at: "seatmap.example".into(),
            doing: "".into(),
            why: Why::Fenced,
            tier: Tier::Work,
            count: 1,
        },
        IloKind::Yours { tab: 1, site: None },
        IloKind::Yours {
            tab: 1,
            site: Some("boxoffice.example".into()),
        },
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
        IloKind::YouStopped { pages: 0 },
        IloKind::YouStopped { pages: 2 },
        IloKind::StartedWithout {
            left: LeftOut::Folder("Trip".into()),
        },
        IloKind::StartedWithout {
            left: LeftOut::Files(3),
        },
        IloKind::KeptAt {
            at: Tier::Work,
            refused: Tier::Full,
            folder: "Trip".into(),
        },
        IloKind::SetBack {
            at: Tier::Work,
            refused: Tier::Full,
            folder: "Trip".into(),
        },
        IloKind::NotRestarted {
            left: LeftOut::Folder("Trip".into()),
        },
        IloKind::NotRestarted {
            left: LeftOut::Files(3),
        },
        IloKind::NotRaised {
            refused: Tier::Full,
        },
        IloKind::LeftRunning {
            reach: Reach::Folder("Trip".into()),
        },
        IloKind::LeftRunning {
            reach: Reach::Files,
        },
        IloKind::EndedLeft,
        IloKind::Revoked {
            folder: "Trip".into(),
        },
        IloKind::FolderGone {
            folder: "Trip".into(),
        },
        IloKind::NotCopied {
            left: LeftOut::File("seats.pdf".into()),
        },
        IloKind::NotCopied {
            left: LeftOut::Files(3),
        },
        IloKind::Resumed { tier: Tier::Work },
        IloKind::KeptHere,
        IloKind::Gone,
        IloKind::NotKept,
        IloKind::KeepsNothing,
        IloKind::NoProfile,
        IloKind::NoSpace,
        IloKind::Helpers {
            tab: 1,
            limit: HelpersLimit::Count,
        },
        IloKind::Helpers {
            tab: 1,
            limit: HelpersLimit::Depth,
        },
        IloKind::Helpers {
            tab: 1,
            limit: HelpersLimit::Kind,
        },
    ];
    for kind in &every {
        // Every kind is in the list above: a new one fails to compile here until it's added.
        match kind {
            IloKind::Stopped { .. }
            | IloKind::Yours { .. }
            | IloKind::TabBack { .. }
            | IloKind::SignedIn { .. }
            | IloKind::TierChanged { .. }
            | IloKind::YouStopped { .. }
            | IloKind::StartedWithout { .. }
            | IloKind::KeptAt { .. }
            | IloKind::SetBack { .. }
            | IloKind::NotRestarted { .. }
            | IloKind::NotRaised { .. }
            | IloKind::LeftRunning { .. }
            | IloKind::EndedLeft
            | IloKind::Revoked { .. }
            | IloKind::FolderGone { .. }
            | IloKind::NotCopied { .. }
            | IloKind::Resumed { .. }
            | IloKind::KeptHere
            | IloKind::Gone
            | IloKind::NotKept
            | IloKind::KeepsNothing
            | IloKind::NoProfile
            | IloKind::NoSpace
            | IloKind::Helpers { .. } => {}
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
                doing: "".into(),
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

/// The Focus command brings the keyboard to the newest line's first button, where a key typed at
/// once presses nothing; after PROTECT Tab moves along the line and ↩ presses the ringed button,
/// and Escape or the command again gives the keyboard back, pressing nothing (#3955).
#[gpui::test]
fn the_focus_command_alone_brings_the_keys_to_an_ilo_lines_buttons(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = stopped_at_order(cx);
    let past_protect = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.update(cx, |view, _| {
            let keys = view.ilo_keys.as_mut().expect("the keys left the line");
            keys.last -= crate::agents::tier::PROTECT * 2;
            // The buttons have stood, as a click needs too.
            for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
                *since -= std::time::Duration::from_secs(5);
            }
        });
    };
    let pressed = |host: &Rc<Recorder>| -> Vec<String> {
        (host.take().into_iter())
            .filter(|line| line.contains("SetTier") || line.contains("TakeOver"))
            .collect()
    };
    host.take();
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "ui", cx)
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        pressed(&host),
        Vec::<String>::new(),
        "a key typed as the keys came pressed"
    );
    cx.simulate_keystrokes("tab");
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(1));
    past_protect(&view, cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(pressed(&host), ["task task-1 SetTier(Full)"]);

    past_protect(&view, cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "Escape kept the keys"
    );
    // The command again, after it brought them, gives them back too.
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "ui", cx)
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "ui", cx)
    });
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "the key kept the keys"
    );
    assert_eq!(
        pressed(&host),
        Vec::<String>::new(),
        "Escape or the key pressed a button"
    );
}

/// The Focus command, `from` the UI's keys or the page's.
fn focus_command(shell: &Entity<Shell>, from: &str, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, from, cx)
    });
    cx.run_until_parked();
}

/// Task-1's page 30 watched over its conversation from a picture's place, by the user.
pub(super) fn watch_page(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    use gpui::{Bounds, size};
    let thumb = Bounds::new(point(px(400.), px(300.)), size(px(160.), px(100.)));
    shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, thumb, None, false, cx)
    });
    cx.run_until_parked();
}

/// Whether GPUI's focus is on one of the view's ilo buttons' handles.
fn on_an_ilo_button(view: &Entity<super::TalkView>, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let stops = &view.read(cx).ilo_stops;
        stops.iter().any(|stop| stop.is_focused(window))
    })
}

/// Over a page of the task's watched from its conversation the Focus command keys no line, whose
/// buttons aren't in sight: with the keyboard the window's it goes back to the conversation, to
/// its reply field, and rings nothing, and the command again, with the line drawn, is what rings
/// a button.
#[gpui::test]
fn the_focus_command_over_a_page_watched_goes_back_and_keys_no_line(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = stopped_at_order(cx);
    watch_page(&shell, cx);
    host.take();
    focus_command(&shell, "ui", cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, None)),
        "the key didn't go back to the conversation"
    );
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "the key over a page watched keyed a line"
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call == "focus page"),
        "the key gave a page the keyboard: {calls:?}"
    );
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(
        cx.update(|window, _| reply.is_focused(window)),
        "the reply field hasn't the keyboard"
    );
    cx.simulate_keystrokes("enter space");
    assert!(
        !host
            .take()
            .iter()
            .any(|line| line.contains("SetTier") || line.contains("TakeOver")),
        "a key after it pressed an ilo line's button"
    );
    focus_command(&shell, "ui", cx);
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
}

/// A page the user has keeps the view and the key: one they hold from its agent, as a sign-in
/// handed to them or one taken over, whoever has the keyboard, and one with the keyboard. The
/// key from the page stays the page's.
#[gpui::test]
fn the_focus_command_does_nothing_on_a_page_the_user_has(cx: &mut TestAppContext) {
    use crate::agents::Status;
    let (shell, host, view, cx) = stopped_at_order(cx);
    watch_page(&shell, cx);
    let nothing = |why: &str, cx: &mut VisualTestContext| {
        host.take();
        focus_command(&shell, "ui", cx);
        focus_command(&shell, "page", cx);
        assert_eq!(host.take(), Vec::<String>::new(), "{why}");
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, Some(30))),
            "{why}: the page was left"
        );
        assert!(
            view.read_with(cx, |view, _| view.ilo_keys.is_none()),
            "{why}: a line was keyed"
        );
        for key in ["alt-shift-cmd-a", "ctrl-shift-a"] {
            assert!(
                !shell.update(cx, |shell, cx| shell.run_ilo_page_key(key, false, cx)),
                "{why}: the key was kept from the page"
            );
        }
    };
    // The page has the keyboard, as after a click in it.
    host.page_keys.set(Some(true));
    nothing("the page has the keyboard", cx);
    // The user holds it, with the keyboard in the UI, as after a click in the sidebar.
    host.page_keys.set(Some(false));
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(30, "Sign in".into())],
        taken: Vec::new(),
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    cx.run_until_parked();
    nothing("the user holds the page", cx);
}

/// The keys on a line end the moment its conversation is no longer what the area draws, under a
/// page watched over it or with the area closed, and never come back by themselves: drawn again,
/// no button is ringed and no key presses one until the Focus command, whose guard counts from
/// then.
#[gpui::test]
fn the_keys_end_as_the_conversation_stops_being_drawn(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = stopped_at_order(cx);
    let pressed = |host: &Rc<Recorder>| -> Vec<String> {
        (host.take().into_iter())
            .filter(|line| line.contains("SetTier") || line.contains("TakeOver"))
            .collect()
    };
    let stood = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.update(cx, |view, _| {
            for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
                *since -= std::time::Duration::from_secs(5);
            }
        });
    };
    let unkeyed = |what: &str, cx: &mut VisualTestContext| {
        assert!(
            view.read_with(cx, |view, _| view.ilo_keys.is_none()),
            "{what}: the keys stayed"
        );
        assert!(
            !on_an_ilo_button(&view, cx),
            "{what}: a button kept the focus"
        );
    };
    focus_command(&shell, "ui", cx);
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    watch_page(&shell, cx);
    unkeyed("a page watched over it", cx);
    // Back at the conversation, long after: nothing is keyed, and no key presses.
    shell.update(cx, |shell, cx| shell.task_page_back(cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), None);
    unkeyed("drawn again", cx);
    stood(&view, cx);
    host.take();
    cx.simulate_keystrokes("enter space");
    assert_eq!(pressed(&host), Vec::<String>::new(), "a key pressed");
    // The command keys it again, and a key at once presses nothing: the guard counts from here.
    focus_command(&shell, "ui", cx);
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    cx.simulate_keystrokes("enter");
    assert_eq!(
        pressed(&host),
        Vec::<String>::new(),
        "the guard didn't count"
    );

    // The area closes with the keys on the line, and shows the conversation again.
    shell.update(cx, |shell, cx| shell.close_task(cx));
    cx.run_until_parked();
    unkeyed("the area closed", cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), None);
    unkeyed("shown again", cx);
    stood(&view, cx);
    host.take();
    cx.simulate_keystrokes("enter space");
    assert_eq!(pressed(&host), Vec::<String>::new(), "a key pressed");
}

/// Keys that end as a window of the app's own takes the keyboard, as the command bar opened over
/// the line, leave GPUI's focus where the key again would have put the keyboard, on what the
/// window draws: its keys are heard once the keyboard is back, the Focus command's too, with no
/// click (#3869). Nothing is keyed by itself, and no key presses.
#[gpui::test]
fn keys_ended_by_a_window_over_the_line_leave_the_windows_keys_heard(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = stopped_at_order(cx);
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    assert!(cx.update(|window, _| reply.is_focused(window)));
    focus_command(&shell, "ui", cx);
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    assert!(on_an_ilo_button(&view, cx));
    host.app_active.set(true);
    host.take();
    shell.update(cx, |shell, cx| shell.ilo_keys_window_inactive(cx));
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "the keys stayed under the window"
    );
    assert!(
        cx.update(|window, _| reply.is_focused(window)),
        "the focus isn't where the keyboard came from"
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("focus")),
        "the keyboard moved under the window: {calls:?}"
    );
    view.update(cx, |view, _| {
        for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
            *since -= std::time::Duration::from_secs(5);
        }
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), None);
    assert!(
        !host
            .take()
            .iter()
            .any(|line| line.contains("SetTier") || line.contains("TakeOver")),
        "a key after the window pressed an ilo line's button"
    );
    // The command's own key, through the window's bindings.
    cx.simulate_keystrokes("alt-shift-cmd-a");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.ilo_ring(7)),
        Some(0),
        "the Focus command's key wasn't heard"
    );
}

/// A line that comes after the ring was drawn on another line's button is never pressed by the
/// key meant for the ringed one: the keys end, and nothing is pressed.
#[gpui::test]
fn a_line_arriving_after_the_ring_takes_no_press(cx: &mut TestAppContext) {
    use super::ilo_keys::IloPress;
    let (shell, host, view, cx) = stopped_at_order(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "ui", cx)
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    let mut talk = view.read_with(cx, |view, _| view.shown.clone().unwrap());
    talk.lines.push(Line::Ilo(IloLine {
        key: 8,
        kind: IloKind::Stopped {
            tab: 30,
            at: "Pay".into(),
            doing: "clicking “Pay”".into(),
            why: Why::Message,
            tier: Tier::Work,
            count: 2,
        },
        settled: false,
    }));
    view.update(cx, |view, _| {
        let keys = view.ilo_keys.as_mut().expect("the keys left the line");
        keys.last -= crate::agents::tier::PROTECT * 2;
        for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
            *since -= std::time::Duration::from_secs(5);
        }
    });
    host.take();
    // The line comes and the key with it, before a frame draws the line, the ring still on line
    // 7's button.
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
        view.update(cx, |view, cx| view.ilo_press(&IloPress, window, cx));
    });
    cx.run_until_parked();
    let pressed: Vec<String> = (host.take().into_iter())
        .filter(|line| line.contains("SetTier") || line.contains("TakeOver"))
        .collect();
    assert_eq!(pressed, Vec::<String>::new(), "the new line was pressed");
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "the keys stayed on a line that changed under them"
    );
}

/// What the checks wait on to press by key is the guard's own end: they're told only once a key
/// on the ringed button would press, and once its line has moved under a button that widens they
/// are told anew, again not before.
#[gpui::test]
fn the_checks_are_told_a_key_presses_only_at_the_guards_end(cx: &mut TestAppContext) {
    use super::ilo_keys::{Tell, Told};
    use std::time::{Duration, Instant};
    let (shell, host, view, cx) = stopped_at_order(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::FocusAgentQuestion, "ui", cx)
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(1));
    let guard = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.ilo_guard())
            .expect("no guard with the keys on a line")
    };
    // As if `protect` had passed since the keyboard came, and `stood` since the buttons were drawn.
    let later = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext, stood: Duration| {
        view.update(cx, |view, _| {
            view.ilo_keys.as_mut().unwrap().last -= crate::agents::tier::PROTECT * 2;
            for (_, since) in view.ilo_drawn.borrow_mut().values_mut() {
                *since -= stood;
            }
        });
    };
    let raised = |host: &Rc<Recorder>| host.take().iter().any(|line| line.contains("SetTier"));
    let mut told = Told::default();
    let long = Duration::from_secs(5);
    host.take();

    // The keyboard has been there long enough, the button hasn't stood: neither told nor pressed.
    later(&view, cx, Duration::ZERO);
    let first = guard(&view, cx);
    assert!(matches!(
        told.step(first, Instant::now(), false),
        Tell::Wait(_)
    ));
    cx.simulate_keystrokes("enter");
    assert!(!raised(&host), "a key pressed a button that hadn't stood");
    later(&view, cx, long);
    let stood = guard(&view, cx);
    assert_eq!(told.step(stood, Instant::now(), false), Tell::Ready);
    assert_eq!(
        told.step(stood, Instant::now(), false),
        Tell::Nothing,
        "told twice of one guard"
    );

    // A line comes above it and the button moves: the guard starts anew, and so does the telling.
    let mut talk = view.read_with(cx, |view, _| view.shown.clone().unwrap());
    talk.lines
        .insert(0, Line::Said("Two seats are left.".into()));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(1));
    let moved = guard(&view, cx);
    assert_ne!(moved, stood, "the button didn't move with its line");
    let Tell::Wait(end) = told.step(moved, Instant::now(), false) else {
        panic!("told ready of a button that had just moved");
    };
    cx.simulate_keystrokes("enter");
    assert!(!raised(&host), "a key pressed a button that had just moved");
    // Its wait ends at the guard's end, and a moment short of it nothing is told.
    let moved = guard(&view, cx);
    let Tell::Wait(end_again) = told.step(moved, Instant::now(), false) else {
        panic!("told ready after a key that came too soon");
    };
    assert!(end_again >= end, "a key didn't push the guard's end back");
    let short = Duration::from_millis(2);
    assert!(matches!(
        told.step(moved, end_again - short, true),
        Tell::Wait(_)
    ));
    assert_eq!(told.step(moved, end_again + short, true), Tell::Ready);
    // And once that long has passed, the key presses.
    later(&view, cx, long);
    cx.simulate_keystrokes("enter");
    assert!(raised(&host), "a key after the guard's end didn't press");
}

/// What the checks read to press a line's button is the line's key, the button's own name and its
/// place: nothing the line says, which may be a page's words or a site, is in it.
#[gpui::test]
fn a_buttons_place_for_the_checks_holds_no_word_of_its_line(cx: &mut TestAppContext) {
    use crate::agents::task::ilo;
    let (_shell, _host, view, cx) = stopped_at_order(cx);
    let order = view.read_with(cx, |view, _| {
        (view.shown.as_ref().unwrap().lines.iter())
            .find_map(|line| match line {
                Line::Ilo(line) => Some(line.kind.clone()),
                _ => None,
            })
            .expect("no ilo line")
    });
    // The same stop at another site's door, where the line says the site.
    let mut site = order.clone();
    if let IloKind::Stopped { at, why, .. } = &mut site {
        *at = "seatmap.example".into();
        *why = Why::OtherSite;
    }
    let yours = IloKind::Yours {
        tab: 30,
        site: Some("boxoffice.example".into()),
    };
    let bounds = gpui::Bounds::new(point(px(12.), px(340.)), gpui::size(px(96.), px(24.)));
    for kind in [order, site, yours] {
        let (own, name) = super::words::head(&kind, "Claude");
        let said = format!("{own} {}", name.unwrap_or_default());
        let line = IloLine {
            key: 7,
            kind,
            settled: false,
        };
        let buttons = ilo::drawn(&line, Some(30));
        assert!(!buttons.is_empty(), "{said}: no button to place");
        for button in buttons {
            let name = super::words::button(button);
            let place = super::ilo_keys::place((7, button), bounds);
            let rest = (place.strip_prefix("browser: ilo line 7 ")).expect("no line's key");
            let (named, at) = rest.rsplit_once(" at ").expect("no place");
            assert_eq!(named, name, "more than the button's name: {place}");
            assert_eq!(at, "12 340 96 24");
            for word in said.split(|each: char| !each.is_alphanumeric() && each != '.') {
                let own = name.to_lowercase().contains(&word.to_lowercase());
                assert!(
                    word.len() < 4 || own || !place.contains(word),
                    "“{word}” of “{said}” is in {place}"
                );
            }
        }
    }
}

/// The reply field's caret stands before its placeholder, not on its first letter; what's typed
/// starts at the field's start, where the caret was.
#[gpui::test]
fn the_reply_fields_caret_stands_before_its_placeholder(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let reply = shell.read_with(cx, |shell, cx| {
        shell.talk.as_ref().unwrap().view.read(cx).reply.clone()
    });
    let inset = reply.read_with(cx, |reply, _| reply.placeholder_inset());
    assert!(
        inset > crate::text_input::CARET,
        "the placeholder starts {inset:?} in, under the caret"
    );
    cx.update(|window, cx| {
        reply.update(cx, |reply, cx| reply.set_text("ok".into(), 2, window, cx))
    });
    assert_eq!(
        reply.read_with(cx, |reply, _| reply.placeholder_inset()),
        px(0.)
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
        live: Default::default(),
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
        live: Default::default(),
        ..conversation(0)
    }
}

/// Where the words of ilo's line 7 were drawn and how they were laid out, and where the
/// conversation was, once it has drawn again.
pub(super) fn ilo_words(
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
        laid.text().ends_with("still has this conversation."),
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
        doing: "clicking “Place the order for both of the seats”".into(),
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

/// The chip under the reply field says the task's own level, at Look only and at Full access,
/// and a click on it opens the level's menu: names only, that level checked, where it may go and
/// Settings. A pick there is the user's press on the task, the one way its level changes.
#[gpui::test]
fn the_level_chip_says_the_tasks_level_and_opens_its_menu(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    for tier in [Tier::Look, Tier::Full] {
        shell.update(cx, |shell, cx| {
            shell.set_talk_beside(true, cx);
            shell.set_conversation(
                Some(Conversation {
                    tier,
                    ..ended(IloKind::KeptHere)
                }),
                cx,
            );
        });
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        let chip = view.read_with(cx, |view, _| view.level_drawn.get());
        let chip = chip.expect("the level chip drawn");
        host.take();
        cx.simulate_click(chip.center(), Modifiers::none());
        cx.run_until_parked();
        let rows: Vec<(String, bool)> = (host.menus.borrow().last().cloned())
            .expect("a click on the chip showed no menu")
            .into_iter()
            .map(|(title, enabled)| (title.to_string(), enabled))
            .collect();
        let row = |title: &str, enabled: bool| (title.to_string(), enabled);
        assert_eq!(
            rows,
            vec![
                row("Look only", true),
                row("Work on pages", true),
                row("Full access", true),
                row("", false),
                row("Any site", true),
                row("Only the sites you add", false),
                row("", false),
                row("Settings…", true),
            ],
            "the level menu's rows at {tier:?}"
        );
        let checked: Vec<bool> = host.entries.borrow().iter().map(|e| e.checked).collect();
        let at = |each: Tier| tier == each;
        assert_eq!(
            checked,
            [
                at(Tier::Look),
                at(Tier::Work),
                at(Tier::Full),
                false,
                true,
                false,
                false,
                false
            ],
            "the menu checks {tier:?}, the task's own level"
        );
    }
    host.take();
    cx.update(|_, cx| host.choose("Work on pages", cx));
    assert_eq!(
        host.take(),
        vec!["task task-1 PickTier(Work)".to_string()],
        "the pick is the user's press on the task"
    );
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

/// A helper's line on `step`, which its task gave no name.
fn helper(step: TraceStep) -> TraceItem {
    TraceItem::Helper {
        step,
        name: "".into(),
        tab: None,
    }
}

/// A helper its task named says that name on its line, and its page's word after it, as the
/// page's row in the sidebar ends: not its step, and nothing it found. Its page's favicon stands
/// in its step's glyph's place, which a page without one keeps. One its task gave no name has
/// its step's line.
#[test]
fn a_named_helpers_line_says_its_name_and_its_pages_word() {
    use super::{HelperPage, helper_line};
    use crate::shell::task_cards::PageWord;
    let reading = step(10, "Reading shop.example", true);
    let name = gpui::SharedString::from("Northwick Home price");
    for word in [PageWord::Working, PageWord::Done, PageWord::Paused] {
        let page = HelperPage {
            tab: 30,
            title: "Lamps".into(),
            favicon: None,
            word: word.label(),
        };
        let line = helper_line(&reading, &name, Some(&page)).expect("named");
        assert_eq!(
            line.text(),
            format!("Northwick Home price {}", word.label())
        );
        assert_eq!(
            (line.glyph, line.icon.is_some()),
            (Some(reading.glyph), false)
        );
    }
    let favicon = std::sync::Arc::new(gpui::Image::empty());
    let page = HelperPage {
        tab: 30,
        title: "".into(),
        favicon: Some(favicon),
        word: PageWord::Working.label(),
    };
    let line = helper_line(&reading, &name, Some(&page)).expect("named");
    assert_eq!((line.glyph, line.icon.is_some()), (None, true));
    // A page that's gone leaves the name alone.
    let line = helper_line(&reading, &name, None).expect("named");
    assert_eq!(line.text(), "Northwick Home price");
    assert!(helper_line(&reading, &"".into(), Some(&page)).is_none());
}

/// A started helper its task gave no name has no empty line: its row appears with its first
/// words, its page's title once it has a page with one, said as a name is with the page's word
/// after it. Until then it draws nothing, and its stretch's count alone shows it. One with a step
/// and no titled page has its step's line.
#[test]
fn a_helper_with_no_name_has_no_line_until_its_page_has_a_title() {
    use super::{HelperPage, HelperRow, helper_row};
    use crate::shell::task_cards::PageWord;
    let none = gpui::SharedString::default();
    let page = |title: &'static str| HelperPage {
        tab: 30,
        title: title.into(),
        favicon: None,
        word: PageWord::Working.label(),
    };
    // Started, with no step yet: its line has no words of its own.
    let began = step(10, "", true);
    assert!(matches!(
        helper_row(&began, &none, None),
        HelperRow::Nothing
    ));
    let untitled = page("");
    let row = helper_row(&began, &none, Some(&untitled));
    assert!(matches!(row, HelperRow::Nothing));
    let titled = page("Lamps · Northwick Home");
    for on in [&began, &step(11, "Reading shop.example", true)] {
        let HelperRow::Known(line) = helper_row(on, &none, Some(&titled)) else {
            panic!("its page has a title");
        };
        assert_eq!(line.text(), "Lamps · Northwick Home Working");
    }
    // Its count is its stretch's, with or without a line.
    let trace = Trace {
        key: 1,
        running: true,
        took: None,
        items: vec![helper(began.clone()), helper(began)],
    };
    assert_eq!(trace.helpers(), 2);
    // With a step and no titled page, the step's line.
    let reading = step(11, "Reading shop.example", true);
    assert!(matches!(helper_row(&reading, &none, None), HelperRow::Step));
    // Named, it has its line from its start.
    let row = helper_row(&step(10, "", true), &"Lamp prices".into(), None);
    assert!(matches!(row, HelperRow::Known(line) if line.text() == "Lamp prices"));
}

/// Task-1's conversation as it works with `helpers` helpers (#4102), each reading its shop's
/// page, and its own step under way after them.
fn with_helpers(helpers: usize) -> Conversation {
    let mut talk = conversation(1);
    let items = (0..helpers)
        .map(|at| helper(step(10 + at, "Reading shop.example", true)))
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

/// A stretch's summary line draws the one string its task's card writes, its dot between how long
/// it worked and what it counts: under way and over, steps and helpers, one and several. A step's
/// line has no dot before its note.
#[test]
fn a_stretchs_summary_line_draws_the_cards_string() {
    use super::{TraceLine, summary_line};
    for helpers in [0, 1, 4] {
        let mut talk = with_helpers(helpers);
        let trace = its_trace(&mut talk);
        assert_eq!(summary_line(trace).text(), trace.line().as_ref());
        (trace.running, trace.took) = (false, Some(41));
        let drawn = summary_line(trace).text();
        assert_eq!(drawn, trace.line().as_ref());
        assert!(drawn.contains(" \u{b7} "), "{drawn}");
    }
    let step = TraceLine::new(Fold::Step(1), "Read".into(), "shop.example".into());
    assert_eq!(step.text(), "Read shop.example");
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
    its_trace(&mut talk).items[1] = helper(step(11, "Read shop.example", false));
    its_trace(&mut talk).items[2] = helper(step(12, "Handed you shop.example", false));
    assert_eq!(summary(&mut talk), "Working · 4 helpers");

    let over = its_trace(&mut talk);
    (over.running, over.took) = (false, Some(41));
    assert_eq!(summary(&mut talk), "Worked for 41 s · 4 helpers");
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
    its_trace(&mut talk).items[0] = helper(step(10, "Read shop.example", false));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    draw(cx);
    assert_eq!(listed(cx), Some(6. * TRACE_LINE));

    view.update(cx, |view, cx| view.toggle(Fold::Trace(0), true, cx));
    draw(cx);
    assert_eq!(listed(cx), None, "folded, it still listed them");
}

/// In the one view the live line stands over the reply field with Take Over at its end, and
/// names the step under way only: once that step has ended, as at Hand Back, or was refused, it
/// says the assistant is working, until the next step starts (c, h).
#[gpui::test]
fn the_live_line_over_the_reply_names_only_the_step_under_way(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(3)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        view.read_with(cx, |view, _| view.take_over_drawn.borrow().clone())
    };
    let chips = draw(cx);
    assert_eq!(
        chips.len(),
        1,
        "Take Over stands once, on the live line: {chips:?}"
    );
    let reply = view
        .read_with(cx, |view, _| view.reply_drawn.get())
        .expect("the reply field");
    assert!(
        chips[0].bottom() <= reply.top(),
        "the live line isn't over the reply field: {chips:?} {reply:?}"
    );
    host.take();
    cx.simulate_click(chips[0].center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 TakeOver(30)"]);

    let said = |talk: &Conversation| super::under_way(&talk.lines).map(|step| step.title.clone());
    assert_eq!(
        said(&conversation(3)).as_deref(),
        Some("Opened skyscanner.net")
    );
    // The step handed over has ended once the user hands the page back.
    let mut ended = conversation(3);
    its_trace(&mut ended).items[2] = TraceItem::Step(step(2, "Handed you skyscanner.net", false));
    assert_eq!(said(&ended), None);
    // A refused step is never the one under way.
    let mut refused = conversation(3);
    let mut row = step(2, "Couldn't click “Pay”", false);
    row.failed = true;
    its_trace(&mut refused).items[2] = TraceItem::Step(row);
    assert_eq!(said(&refused), None);
    assert_eq!(super::words::working("Claude"), "Claude is working");
    shell.update(cx, |shell, cx| shell.set_conversation(Some(refused), cx));
    assert_eq!(
        draw(cx).len(),
        1,
        "Take Over left the live line between steps"
    );
}

/// While the task waits at a refused or a handed step, or on its own question, no live line
/// stands, as ilo's line with its own buttons is the last over the field; on a page the user took
/// over it says they're driving, with Hand Back, which goes as the ilo line's does. Never Take
/// Over or working then, whatever step its trace has under way.
#[gpui::test]
fn the_live_line_never_says_working_while_the_task_needs_the_user(cx: &mut TestAppContext) {
    use crate::agents::task::Live;
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(3)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let yours = |site: Option<&str>| Live::Yours {
        tab: 30,
        site: site.map(Into::into),
    };
    for live in [Live::Refused, Live::Handed, Live::Asked, yours(None)] {
        let talk = Conversation {
            live: live.clone(),
            ..conversation(3)
        };
        assert!(super::under_way(&talk.lines).is_some(), "no step under way");
        shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        let chips = view.read_with(cx, |view, _| view.take_over_drawn.borrow().len());
        assert_eq!(chips, 0, "Take Over stood on the live line at {live:?}");
        let hand_back = view.read_with(cx, |view, _| view.hand_back_drawn.get());
        assert_eq!(
            hand_back.is_some(),
            matches!(live, Live::Yours { .. }),
            "{live:?}"
        );
    }
    assert_eq!(super::words::live(&Live::Refused), None);
    assert_eq!(super::words::live(&Live::Handed), None);
    assert_eq!(
        super::words::live(&yours(None)).as_deref(),
        Some("You're driving")
    );
    assert_eq!(
        super::words::live(&yours(Some("maat.pt"))).as_deref(),
        Some("You're driving maat.pt")
    );
    let hand_back = view.read_with(cx, |view, _| view.hand_back_drawn.get().unwrap());
    host.take();
    cx.simulate_click(hand_back.center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 HandBack(30)"]);
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
        its_trace(&mut talk).items[at] = helper(step(10 + at, "Read shop.example", false));
    }
    its_trace(&mut talk).items[3] = helper(step(13, "Handed you shop.example", false));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    assert!(!moving(cx), "nothing is under way");
    cx.update(|_, cx| cx.set_reduce_motion(true));
}

/// Its pages show live in their pictures in the rail, where the core can (#4115): a tile's click
/// watches its page as that tile grown to the area, never the window's page (#3788), which goes
/// back to its picture once the page is back, and every tile ends as the task closes.
#[gpui::test]
fn its_pages_are_live_in_the_rail_and_end_as_it_goes(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        host.take()
    };
    let did = draw(cx);
    assert!(
        did.iter().any(|line| line.starts_with("tile 30 at ")),
        "not live in the rail: {did:?}"
    );

    let (_, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    cx.simulate_click(at.center(), Modifiers::none());
    let did = draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30)))
    );
    assert!(
        did.iter().any(|line| line.starts_with("tile 30 at ")),
        "the watch didn't grow the tile: {did:?}"
    );
    assert!(
        !did.contains(&"end tile 30".to_string()) && !did.contains(&"select 30".to_string()),
        "the watched page left its tile: {did:?}"
    );

    cx.simulate_keystrokes("escape");
    let did = draw(cx);
    assert!(
        did.iter().any(|line| line.starts_with("tile 30 at ")),
        "not back in its picture with its page back: {did:?}"
    );

    shell.update(cx, |shell, cx| shell.close_task(cx));
    cx.run_until_parked();
    assert!(
        host.take().contains(&"end tile 30".to_string()),
        "live with the task closed"
    );
}

/// A close a page of the user's refuses brings a page watched in its tile back in that tile grown
/// to the area (#3788, #4202): it goes back to its picture for the close, and the window never
/// shows the agent's tab.
#[gpui::test]
fn a_refused_close_brings_the_watched_tile_back(cx: &mut TestAppContext) {
    use crate::shell::{DialogKind, PageDialog, PageReply};
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        shell.update(cx, |_, cx| cx.notify());
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        host.take()
    };
    draw(cx);
    let (_, at) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    cx.simulate_click(at.center(), Modifiers::none());
    draw(cx);

    shell.update(cx, |shell, cx| shell.run(Command::CloseWindow, "ui", cx));
    let did = draw(cx);
    assert!(
        !did.contains(&"end tile 30".to_string()),
        "the tile ended for the close: {did:?}"
    );
    let dialog = PageDialog {
        id: 7,
        kind: DialogKind::Leave,
        title: "".into(),
        message: "".into(),
        text: "".into(),
        accept: "Leave".into(),
        cancel: "Stay".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    };
    shell.update(cx, |shell, cx| shell.set_page_dialog(10, Some(dialog), cx));
    draw(cx);
    let stay = PageReply {
        accept: false,
        text: String::new(),
        password: String::new(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 7, stay, cx));
    let did = draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30))),
        "Stay didn't bring the page watched back: {did:?}"
    );
    assert!(
        did.iter().any(|line| line.starts_with("tile 30 at ")),
        "the page watched isn't its tile again: {did:?}"
    );
    assert!(
        !did.contains(&"select 30".to_string()),
        "the window showed the agent's tab: {did:?}"
    );
}

/// Only a picture wholly in the rail's view is live: where the row under the header holds more
/// than fit, those past its edge keep their pictures until it scrolls to them, and those it
/// scrolls away from end.
#[gpui::test]
fn only_a_picture_wholly_in_the_rails_view_is_live(cx: &mut TestAppContext) {
    use crate::tokens::shell::task::RAIL_BREAKPOINT;
    use gpui::size;
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    cx.simulate_resize(size(px(RAIL_BREAKPOINT - 100.), px(900.)));
    let mut talk = conversation(1);
    talk.tabs = (30..36)
        .map(|tab| (tab, "tickets.example".into()))
        .collect();
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let live = |cx: &mut VisualTestContext| {
        for _ in 0..2 {
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
        let did = host.take();
        let shown = |tab: u64| {
            did.iter()
                .any(|line| line.starts_with(&format!("tile {tab} at ")))
        };
        let ended = |tab: u64| did.contains(&format!("end tile {tab}"));
        (did.clone(), shown(30), shown(35), ended(30))
    };
    let (did, first, last, _) = live(cx);
    assert!(first, "the first picture isn't live: {did:?}");
    assert!(!last, "a picture past the row's edge is live: {did:?}");

    view.update(cx, |view, _| {
        view.rail_scroll
            .set_offset(gpui::point(px(-10_000.), px(0.)))
    });
    let (did, _, last, first_ended) = live(cx);
    assert!(last, "the last picture isn't live, scrolled to: {did:?}");
    assert!(
        first_ended,
        "the first picture, scrolled away, is still live: {did:?}"
    );
}

/// Every door to a page goes through its tile where the core shows them, as the sidebar's row and
/// the strip's key stop do through watch_task_page: the page only watched is never the window's.
/// A handoff while the user types in the reply field beside it leaves the field its keyboard and
/// its words, and asks no "focus page".
#[gpui::test]
fn a_watch_from_any_door_is_a_tile_and_a_handoff_leaves_the_reply_field(cx: &mut TestAppContext) {
    use crate::agents::Status;
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        host.take()
    };
    draw(cx);

    assert!(shell.update(cx, |shell, cx| {
        shell.watch_task_page(30, Default::default(), None, false, cx)
    }));
    let did = draw(cx);
    assert!(
        did.iter().any(|line| line.starts_with("tile 30 at ")),
        "the watch didn't grow the tile: {did:?}"
    );
    assert!(
        !did.contains(&"select 30".to_string()),
        "a page only watched is the window's: {did:?}"
    );

    shell.update(cx, |shell, cx| shell.set_talk_beside(true, cx));
    let reply = view.read_with(cx, |view, cx| view.reply.focus_handle(cx));
    cx.update(|window, cx| window.focus(&reply, cx));
    cx.simulate_input("Ryanair");
    draw(cx);
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(30, "Sign in".into())],
        taken: Vec::new(),
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    let mut did = draw(cx);
    shell.update(cx, |shell, _| shell.age_watch_press());
    did.extend(draw(cx));
    assert!(
        !did.iter().any(|line| line.starts_with("focus page")),
        "{did:?}"
    );
    assert!(cx.update(|window, _| reply.is_focused(window)), "{did:?}");
    assert_eq!(
        view.read_with(cx, |view, cx| view.reply.read(cx).text().to_string()),
        "Ryanair"
    );
}

/// Where task-1's folder chip under the reply field was drawn, with its conversation `talk`.
fn folder_chip_with(
    talk: Conversation,
    cx: &mut TestAppContext,
) -> (
    Option<gpui::Bounds<gpui::Pixels>>,
    Rc<Recorder>,
    &mut VisualTestContext,
) {
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_talk_beside(true, cx);
        shell.set_conversation(Some(talk), cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let drawn = view.read_with(cx, |view, _| view.folder_drawn.get());
    (drawn, host, cx)
}

/// The folder a live task holds is under its reply field by its name, and its one act, Revoke,
/// takes it back (#4101). A reply carries no files or folder, so nothing there adds one.
#[gpui::test]
fn the_folder_a_task_holds_is_under_its_reply_field_with_revoke(cx: &mut TestAppContext) {
    let talk = Conversation {
        folder: Some("Notes".into()),
        ..conversation(0)
    };
    let (drawn, host, cx) = folder_chip_with(talk, cx);
    let chip = drawn.expect("the folder's chip under the reply field");
    host.take();
    cx.simulate_click(chip.center(), Modifiers::none());
    let menu = host
        .menus
        .borrow()
        .last()
        .cloned()
        .expect("the chip's menu");
    assert_eq!(menu, vec![("Revoke".into(), true)], "the folder's one act");
    cx.update(|_, cx| host.choose("Revoke", cx));
    assert!(
        host.take()
            .contains(&"task task-1 RevokeFolder".to_string()),
        "Revoke didn't take the task's folder back"
    );
}

/// A task that holds no folder has no folder chip.
#[gpui::test]
fn a_task_without_a_folder_has_no_folder_chip(cx: &mut TestAppContext) {
    let (drawn, _, _) = folder_chip_with(conversation(0), cx);
    assert!(drawn.is_none(), "a folder chip for a task that holds none");
}

/// A conversation that closes leaves its card in Latest tasks the picture its page's tile ended
/// on, from memory, and lets the rest go; the card has none while the rail holds it, and none
/// once it pictures another page, or is gone.
#[gpui::test]
fn a_closed_conversation_leaves_its_card_the_picture_its_tile_ended_on(cx: &mut TestAppContext) {
    let card = Card {
        last_tab: Some(30),
        pages: vec![30],
        ..card()
    };
    let (shell, host, cx) = shown_with(card.clone(), cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let plate = |shell: &Entity<Shell>, card: &Card, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.plate_of(card).map(|p| p.size()))
    };
    assert_eq!(plate(&shell, &card, cx), None, "a plate before any picture");
    let request = picture_request(&shell, 30, cx);
    let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
    shell.update(cx, |shell, cx| shell.page_pictured(request, picture, cx));
    assert_eq!(
        plate(&shell, &card, cx),
        None,
        "the rail holds it, not the card"
    );

    shell.update(cx, |shell, cx| shell.close_task(cx));
    assert_eq!(
        plate(&shell, &card, cx),
        Some((2., 2.)),
        "the card has no picture"
    );

    // The task goes on to another page: its card no longer pictures this one.
    let on = Card {
        last_tab: Some(31),
        pages: vec![30, 31],
        ..card.clone()
    };
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![on.clone()], cx));
    assert_eq!(plate(&shell, &card, cx), None);
    assert_eq!(plate(&shell, &on, cx), None, "another page's picture");
    let held = shell.read_with(cx, |shell, _| shell.card_plates.len());
    assert_eq!(held, 0, "a picture no card shows is still held");
}

/// Tab 30's page as the core reports it with `trouble`.
fn page_in(trouble: Trouble) -> Page {
    Page {
        url: "https://skyscanner.net/".into(),
        trouble,
        ..Page::default()
    }
}

/// A page that died stands in the rail under the crash card's glyph, in its picture's middle
/// (#3433). Its tile stays the core's, which hides it and shows it again: nothing here ends it or
/// pictures it anew, so no page is asked to draw there, and a click still shows the page large,
/// where the card is. Once the page loads again, whoever loaded it, the glyph goes and the tile
/// is the live one it was.
#[gpui::test]
fn a_rail_tile_whose_page_died_has_the_cards_glyph_until_it_loads_again(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        host.take()
    };
    let glyph = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            let drawn = view.crashed_drawn.borrow();
            drawn.iter().find(|(tab, _)| *tab == 30).map(|(_, at)| *at)
        })
    };
    let asked = |did: &[String], what: &str| did.iter().any(|line| line.starts_with(what));
    let did = draw(cx);
    assert!(asked(&did, "tile 30 at "), "not live in the rail: {did:?}");
    assert_eq!(glyph(cx), None, "a live page's tile has the glyph");

    shell.update(cx, |shell, cx| {
        shell.set_page(30, page_in(Trouble::Crashed), cx)
    });
    let did = draw(cx);
    let (_, tile) = view.read_with(cx, |view, _| view.drawn.borrow()[0]);
    let at = glyph(cx).expect("a dead page's tile has no glyph");
    // To the half point either way, as the glyph is laid out on whole points and a tile isn't.
    let off = at.center() - tile.center();
    assert!(
        off.x.abs() <= px(0.5) && off.y.abs() <= px(0.5),
        "not in its picture's middle: {at:?} in {tile:?}"
    );
    assert!(
        !asked(&did, "tile 30 at ")
            && !asked(&did, "end tile 30")
            && !asked(&did, "task picture 30"),
        "a dead page's tile was moved, ended or pictured: {did:?}"
    );

    cx.simulate_click(tile.center(), Modifiers::none());
    let did = draw(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30))),
        "a click on a dead page's tile didn't show it large"
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.crash_cards.contains_key(&30)),
        "shown large, the dead page has no card"
    );
    assert!(!asked(&did, "end tile 30"), "the tile ended: {did:?}");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(
        glyph(cx).is_some(),
        "back in the rail, still dead, no glyph"
    );

    shell.update(cx, |shell, cx| {
        shell.set_page(30, page_in(Trouble::Fine), cx)
    });
    let did = draw(cx);
    assert_eq!(
        glyph(cx),
        None,
        "the glyph stayed on a page that loaded again"
    );
    assert!(
        !asked(&did, "end tile 30"),
        "loaded again, its tile ended: {did:?}"
    );
}

/// The picture a dead page's tile ends on comes back empty, as there's no page to picture
/// (#3433): the rail keeps the one it had, which the wash and the glyph stand over.
#[gpui::test]
fn an_empty_picture_at_a_dead_tiles_end_leaves_the_one_the_rail_had(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    host.shows_tiles.set(Some(true));
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let request = picture_request(&shell, 30, cx);
    let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
    shell.update(cx, |shell, cx| shell.page_pictured(request, picture, cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let held = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            let held = view.pictures.iter().find(|(tab, ..)| *tab == 30);
            held.and_then(|(.., picture)| picture.as_ref().map(PagePicture::size))
        })
    };
    assert_eq!(held(cx), Some((2., 2.)));

    shell.update(cx, |shell, cx| {
        shell.set_page(30, page_in(Trouble::Crashed), cx)
    });
    let tiles = view.read_with(cx, |view, _| view.tiles.clone());
    cx.update(|_, cx| tiles.end_all(cx));
    let asked = tiles.asked_at_ends();
    assert_eq!(asked.len(), 1, "the tile's end asked for no picture");
    shell.update(cx, |shell, cx| shell.page_pictured(asked[0], None, cx));
    assert_eq!(
        held(cx),
        Some((2., 2.)),
        "an empty picture took the place of the one the rail had"
    );
}
