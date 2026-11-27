//! A task's card and its conversation in the page area, on a host that records what the shell asks
//! of the core.

use gpui::EntityInputHandler as _;
use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Entity, Focusable, FontWeight, Modifiers, TestAppContext, VisualTestContext, WindowTextSystem,
    point, px,
};

use super::Fold;

use crate::agents::Status;
use crate::agents::task::ilo::{HelpersLimit, IloButton, IloKind, IloLine, LeftOut, Reach};
use crate::agents::task::{
    Card, Conversation, Dot, Line, Live, Opens, Trace, TraceItem, TraceStep, Word,
};
use crate::agents::tier::{Tier, Why};
use crate::command_bar::choices;
use crate::product::PRODUCT;
use crate::shell::MenuEntry;
use crate::shell::picture::PagePicture;
use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};
use crate::text_input::TextInput;
use crate::tokens::shell::conversation::ilo::TEXT_LINE as ILO_TEXT_LINE;
use crate::tokens::shell::conversation::{INSET, SITE, SITE_GAP, STRETCH_GAP, TRACE_LINE};

/// How tall `lines` of a stretch stand, one over the next with the log's gap between.
fn lines_high(lines: f32) -> f32 {
    lines * TRACE_LINE + (lines - 1.) * STRETCH_GAP
}
use crate::tokens::shell::task_area::BESIDE;
use crate::tokens::text;
use crate::tooltip::text_width;

pub(super) fn card() -> Card {
    Card {
        kept_picture: None,
        agent: "task-1".into(),
        home: 1,
        space: 3,
        name: "Claude".into(),
        replies: true,
        title: "Flight to Lisbon".into(),
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_tab: None,
        line: None,
        ended_at: None,
        asks: false,
        dot: Dot::Working,
        wait: Default::default(),
        word: Word::for_dot(Dot::Working),
        pages: Vec::new(),
        tier: Default::default(),
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
        page: None,
        says: "".into(),
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
        about: format!("{PRODUCT} · Haiku · Working").into(),
        asked: "a flight to Lisbon next Friday".into(),
        page: None,
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            started: None,
            items,
        })],
        tabs: vec![(30, "skyscanner.net".into())],
        on: Some(30),
        held: Vec::new(),
        stoppable: true,
        live: Default::default(),
        open: None,
        answers: Vec::new(),
        writing: false,
        tier: Default::default(),
        finished: Vec::new(),
        folder: None,
        model: None,
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

/// Task-1's conversation shown, its page 30 its card's last and one whose picture the agents'
/// rules let a kept card keep.
fn kept_card_shown(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let card = Card {
        last_tab: Some(30),
        pages: vec![30],
        ..card()
    };
    let (shell, host, cx) = shown_with(card, cx);
    host.keeps.borrow_mut().insert(30);
    (shell, host, cx)
}

/// Whether a kept card may keep a page's picture is the agents' rules', asked as the
/// conversation the Agents send is set, inside the shell's change, so the question carries the
/// page's address. Looked up from the shell there, the app went down at the first thread opened
/// by its card.
#[gpui::test]
fn the_agents_send_a_conversation_whose_pictures_are_judged_before_its_shell_changes(
    cx: &mut TestAppContext,
) {
    let (shell, host, cx) = kept_card_shown(cx);
    host.reads_shell(shell.downgrade());
    host.take();
    let weak = shell.downgrade();
    cx.update(|_, cx| {
        let made = |agent: &str, _: &gpui::App| (agent == "task-1").then(|| conversation(1));
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

/// A picture the conversation asked for comes while the shell changes, as the core answers in
/// the shell's update: whether its card may keep it is still judged, without reading the shell,
/// and the card is told.
#[gpui::test]
fn a_picture_that_comes_is_judged_for_its_card_without_reading_the_shell(cx: &mut TestAppContext) {
    let (shell, host, cx) = kept_card_shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let request = picture_request(&shell, 30, cx);
    host.reads_shell(shell.downgrade());
    shell.update(cx, |shell, cx| {
        let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2);
        shell.page_pictured(request, picture, cx)
    });
    assert_eq!(
        host.card_pictures.borrow_mut().pop(),
        Some(("task-1".into(), 30, true))
    );
}

/// A page watched over the conversation goes back into its picture after its pages changed: the
/// pictures taken again are judged for the card from inside the shell's change, without reading
/// the shell.
#[gpui::test]
fn pictures_taken_again_are_judged_for_the_card_without_reading_the_shell(cx: &mut TestAppContext) {
    let (shell, host, cx) = kept_card_shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |view, _| view.stale = true);
    host.reads_shell(shell.downgrade());
    host.take();
    shell.update(cx, |shell, cx| shell.retake_talk_pictures(cx));
    assert!(
        host.take().contains(&"task picture 30".to_string()),
        "the pictures weren't taken again"
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

/// ↩ that ends a Japanese composition in the reply field is the input method's: it commits the
/// text and sends nothing, and the next ↩ sends what was committed.
#[gpui::test]
fn return_committing_japanese_in_the_reply_field_sends_nothing(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let reply = view.read_with(cx, |view, _| view.reply.clone());
    host.take();
    cx.update(|window, cx| {
        reply.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "にほんご", None, window, cx)
        })
    });
    assert!(reply.read_with(cx, |input, _| input.composing()));
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.contains("Reply")),
        "↩ while composing sent: {calls:?}"
    );
    cx.update(|window, cx| {
        reply.update(cx, |input, cx| {
            input.replace_text_in_range(None, "日本語", window, cx)
        })
    });
    assert!(!reply.read_with(cx, |input, _| input.composing()));
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&r#"task task-1 Reply("日本語")"#.to_string())
    );
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

/// A page ilo opens by itself while the user types in the reply field opens beside the panel, which
/// keeps the field: the field keeps the keyboard and its text.
#[gpui::test]
fn a_needs_you_page_opens_beside_the_reply_field_in_use(cx: &mut TestAppContext) {
    let reply = needs_you_beside_typing(
        |reply, window, cx| reply.set_text("Two seats".into(), 9, window, cx),
        cx,
    );
    assert_eq!(reply, "Two seats");
}

/// A composition under way is typing too, before the input method commits a word of it: the page
/// opening beside it takes no keyboard mid-word. The field holds the marked text in its text.
#[gpui::test]
fn a_needs_you_page_keeps_a_composition_under_way(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let reply = needs_you_beside_typing(
        |reply, window, cx| reply.replace_and_mark_text_in_range(None, "か", None, window, cx),
        cx,
    );
    assert_eq!(reply, "か", "the composition was lost");
}

/// Types into the focused reply field with `type_in`, turns a tile to Needs you, checks ilo
/// opened the page beside the panel and left the field the keyboard, and gives back its text.
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
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    for _ in 0..2 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.watched_task_page().map(|(_, tab)| tab)),
        Some(31),
        "the page that needs the user didn't open beside the panel"
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
    let calls = host.take();
    assert!(
        calls.contains(&"select 31".to_string()),
        "the picture under the page didn't watch it: {calls:?}, {seatmap:?} now {:?}, {:?}",
        picture(31, cx),
        shell.read_with(cx, |shell, _| shell.task_shown()),
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
        started: None,
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
        Some(lines_high(10.)),
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
        opened > lines_high(10.),
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
        started: None,
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

/// The reply field's placeholder follows a switch between light and dark while the conversation
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

/// The reply field says what typing there does (Z-who): `Steer it, or ask` while the thread is at
/// work, `Reply` while it waits on the user and once it can't be stopped, as when it's done.
#[gpui::test]
fn the_reply_field_says_steer_while_at_work_and_reply_otherwise(cx: &mut TestAppContext) {
    use crate::agents::task::Live;
    let (shell, _, cx) = shown(cx);
    let mut talk = conversation(1);
    let mut said = Vec::new();
    for (live, stoppable) in [
        (Live::Working, true),
        (Live::Asked, true),
        (Live::Working, false),
        (Live::Working, true),
    ] {
        talk.live = live;
        talk.stoppable = stoppable;
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(talk.clone()), cx)
        });
        cx.run_until_parked();
        said.push(shell.read_with(cx, |shell, cx| {
            let talk = shell.talk.as_ref().expect("the conversation");
            talk.view.read(cx).reply.read(cx).placeholder().to_string()
        }));
    }
    let (steer, reply) = (super::words::STEER, super::words::REPLY);
    assert_eq!(said, [steer, reply, reply, steer]);
}

/// The line in the reply field's place says where to reply on an outside app's session, and that
/// threads are off while they are, and nothing on a thread that takes a reply; the note under Full
/// access says how long it lasts: the words table's C1 to C3, word for word.
#[test]
fn the_reply_fields_place_and_full_accesss_note_say_their_rows() {
    use super::words::{in_reply_place, note};
    let session = Some("Reply in Claude Code.".to_string());
    assert_eq!(in_reply_place(false, false, "Claude Code"), session);
    assert_eq!(in_reply_place(false, true, "Claude Code"), session);
    let off = Some("Threads are off in Settings › Threads.".to_string());
    assert_eq!(in_reply_place(true, true, "Claude Code"), off);
    assert_eq!(in_reply_place(true, false, "Claude Code"), None);
    assert_eq!(
        note(&[IloButton::Raise(Tier::Full)]),
        Some("For this thread only")
    );
    assert_eq!(note(&[IloButton::Raise(Tier::Work)]), None);
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

/// The line of a run ilo ended over its tools says AP17 as written, and has nothing to press or
/// to say under it.
#[test]
fn a_run_ended_over_its_tools_says_ap17() {
    let kind = IloKind::NotOurTools;
    let said = format!("stopped Claude Code: it started with tools {PRODUCT} doesn't allow");
    assert_eq!(
        super::words::head(&kind, "Claude Code"),
        (said.clone(), None)
    );
    assert_eq!(super::words::stored(&kind, "Claude Code"), said);
    let line = IloLine {
        key: 1,
        kind,
        settled: true,
    };
    assert!(crate::agents::task::ilo::buttons(&line).is_empty());
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
    assert_eq!(laid.text(), format!("{PRODUCT} · You stopped it · 3 pages"));
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
        let own = format!("{PRODUCT} {own}");
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
            "{p} started this thread without \u{201c}Trip\u{201d}: {p} couldn't limit Claude to \
             that folder",
            "\u{201c}Trip\u{201d}",
        ),
        (
            without(LeftOut::File("seats.pdf".into())),
            "{p} started this thread without \u{201c}seats.pdf\u{201d}: {p} couldn't limit Claude \
             to that file",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            without(LeftOut::Files(3)),
            "{p} started this thread without your 3 files: {p} couldn't limit Claude to them",
            "",
        ),
        (
            IloKind::KeptAt {
                at: Tier::Work,
                refused: Tier::Full,
                folder: "Trip".into(),
            },
            "{p} kept this thread at Work on pages: {p} couldn't limit Claude to \
             \u{201c}Trip\u{201d} at Full access",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::SetBack {
                at: Tier::Work,
                refused: Tier::Full,
                folder: "Trip".into(),
            },
            "{p} set this thread back to Work on pages: {p} couldn't limit Claude to \
             \u{201c}Trip\u{201d} at Full access",
            "\u{201c}Trip\u{201d}",
        ),
        (
            stopped(LeftOut::Folder("Trip".into())),
            "{p} stopped this thread: {p} couldn't restart Claude without \u{201c}Trip\u{201d}",
            "\u{201c}Trip\u{201d}",
        ),
        (
            stopped(LeftOut::File("seats.pdf".into())),
            "{p} stopped this thread: {p} couldn't restart Claude without \
             \u{201c}seats.pdf\u{201d}",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            stopped(LeftOut::Files(3)),
            "{p} stopped this thread: {p} couldn't restart Claude without your 3 files",
            "",
        ),
        (
            IloKind::NotRaised {
                refused: Tier::Full,
            },
            "{p} stopped this thread: {p} couldn't restart Claude at Full access",
            "",
        ),
        (
            left(Reach::Folder("Trip".into())),
            "{p} couldn't end everything Claude started: what's left can still reach \
             \u{201c}Trip\u{201d}. Restarting this computer ends it.",
            "\u{201c}Trip\u{201d}",
        ),
        (
            left(Reach::Files),
            "{p} couldn't end everything Claude started: what's left can still reach this \
             thread's files. Restarting this computer ends it.",
            "",
        ),
        (
            left(Reach::Both("Trip".into())),
            "{p} couldn't end everything Claude started: what's left can still reach \
             \u{201c}Trip\u{201d} and this thread's files. Restarting this computer ends it.",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::EndedLeft,
            "{p} ended what Claude had left running",
            "",
        ),
        (
            IloKind::Revoked {
                folder: "Trip".into(),
            },
            "{p} \u{b7} You revoked \u{201c}Trip\u{201d}",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::FolderGone {
                folder: "Trip".into(),
            },
            "{p} revoked \u{201c}Trip\u{201d}: {p} couldn't find it",
            "\u{201c}Trip\u{201d}",
        ),
        (
            IloKind::NotCopied {
                left: LeftOut::File("seats.pdf".into()),
            },
            "{p} started this thread without \u{201c}seats.pdf\u{201d}: {p} couldn't copy it",
            "\u{201c}seats.pdf\u{201d}",
        ),
        (
            IloKind::NotCopied {
                left: LeftOut::Files(3),
            },
            "{p} started this thread without 3 files: {p} couldn't copy them",
            "",
        ),
    ];
    for (kind, whole, name) in cases {
        assert_eq!(
            said(&kind),
            (whole.replace("{p}", PRODUCT), name.to_string()),
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
        IloKind::NotOurTools,
        // A kept log's line, as it stored one of the above.
        IloKind::Kept {
            words: super::words::stored(&IloKind::EndedLeft, "Claude").into(),
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
            | IloKind::Helpers { .. }
            | IloKind::NotOurTools
            | IloKind::Kept { .. } => {}
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

/// Beside a page of the task's watched, the Focus command keys the newest line in the panel, never
/// the page, and the page stays watched; the command again gives the keyboard back.
#[gpui::test]
fn the_focus_command_beside_a_page_watched_keys_the_panels_line(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = stopped_at_order(cx);
    watch_page(&shell, cx);
    host.take();
    focus_command(&shell, "ui", cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(30))),
        "the key left the page"
    );
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), Some(0));
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call == "focus page"),
        "the key gave a page the keyboard: {calls:?}"
    );
    focus_command(&shell, "ui", cx);
    assert!(
        view.read_with(cx, |view, _| view.ilo_keys.is_none()),
        "the key again kept the keys"
    );
}

/// A page the user has stays watched whatever the Focus command does, from the UI's keys or the
/// page's: one they hold from its agent, as a sign-in handed to them, and one with the keyboard.
#[gpui::test]
fn the_focus_command_never_leaves_a_page_the_user_has(cx: &mut TestAppContext) {
    use crate::agents::Status;
    let (shell, host, _, cx) = stopped_at_order(cx);
    watch_page(&shell, cx);
    let stays = |why: &str, cx: &mut VisualTestContext| {
        focus_command(&shell, "ui", cx);
        focus_command(&shell, "page", cx);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.task_shown()),
            Some((3, Some(30))),
            "{why}: the page was left"
        );
    };
    // The page has the keyboard, as after a click in it.
    host.page_keys.set(Some(true));
    stays("the page has the keyboard", cx);
    // The user holds it, with the keyboard in the UI, as after a click in the sidebar.
    host.page_keys.set(Some(false));
    let held = Status {
        agent: "task-1".into(),
        name: "Claude".into(),
        space: 3,
        doing: "".into(),
        held: vec![(30, "Sign in".into())],
    };
    shell.update(cx, |shell, cx| shell.set_agent_status(vec![held], cx));
    cx.run_until_parked();
    stays("the user holds the page", cx);
}

/// The keys on a line end the moment its conversation is no longer what the area draws, as with
/// the area closed, and never come back by themselves: drawn again, no button is ringed and no key
/// presses one until the Focus command, whose guard counts from then.
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
    // The area closes with the keys on the line, and shows the conversation again.
    shell.update(cx, |shell, cx| shell.close_task(cx));
    cx.run_until_parked();
    unkeyed("the area closed", cx);
    shell.update(cx, |shell, cx| shell.show_task("task-1".into(), cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.ilo_ring(7)), None);
    unkeyed("shown again", cx);
    // Long after, no key presses.
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
    // The same stop at an Off limits site's door, where the line says the site.
    let mut site = order.clone();
    if let IloKind::Stopped { at, why, .. } = &mut site {
        *at = "seatmap.example".into();
        *why = Why::Fenced;
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
        shell.set_conversation(Some(ended(kind)), cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    view.read_with(cx, |view, _| view.level_drawn.get().is_some())
}

/// A thread's model whose program can't take another for the next message is its words alone: a
/// click on them opens no menu and doesn't wear a plate.
#[gpui::test]
fn a_model_that_cant_change_opens_nothing(cx: &mut TestAppContext) {
    type Shown = Rc<RefCell<Option<(Vec<MenuEntry>, crate::text_input::Chosen)>>>;
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let shown_menu: Shown = Rc::default();
    let into = shown_menu.clone();
    cx.update(|_, cx| {
        cx.set_global(crate::menu::EntriesMenu(Rc::new(
            move |items, _, _, _, chosen, _| *into.borrow_mut() = Some((items, chosen)),
        )))
    });
    shell.update(cx, |shell, cx| {
        let talk = Conversation {
            model: Some((
                "icons/terminal.svg",
                "Claude Code \u{b7} Opus".into(),
                "Opus".into(),
                false,
            )),
            ..conversation(1)
        };
        shell.set_conversation(Some(talk), cx);
    });
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let words = view.read_with(cx, |view, _| view.model_drawn.get());
    let words = words.expect("the model's words drawn");
    cx.simulate_click(words.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(shown_menu.borrow().is_none(), "a menu showed");
    assert_eq!(view.read_with(cx, |view, _| view.menu_open), None);
}

/// The composer's level trigger says the thread's own level, at Look only and at Full access,
/// and a click on it opens the bar's own level menu (choices.rs) under it, that level checked,
/// wearing its plate while it's open. A pick there is the user's press on the thread, the one way
/// its level changes. Last is the words table's `Settings…`, which opens Settings › Agents at Access.
#[gpui::test]
fn the_level_trigger_says_the_threads_level_and_opens_the_bars_menu(cx: &mut TestAppContext) {
    type Shown = Rc<RefCell<Option<(Vec<MenuEntry>, crate::text_input::Chosen)>>>;
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let shown_menu: Shown = Rc::default();
    let into = shown_menu.clone();
    cx.update(|_, cx| {
        cx.set_global(crate::menu::EntriesMenu(Rc::new(
            move |items, _, _, _, chosen, _| *into.borrow_mut() = Some((items, chosen)),
        )))
    });
    for tier in [Tier::Look, Tier::Full, Tier::Work] {
        shell.update(cx, |shell, cx| {
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
        let trigger = view.read_with(cx, |view, _| view.level_drawn.get());
        let trigger = trigger.expect("the level trigger drawn");
        cx.simulate_click(trigger.center(), Modifiers::none());
        cx.run_until_parked();
        let (items, chosen) = (shown_menu.borrow_mut().take()).expect("no menu showed");
        let rows: Vec<(String, bool, bool)> = (items.iter())
            .map(|item| (item.title.to_string(), item.enabled, item.checked))
            .collect();
        let mut expected: Vec<(String, bool, bool)> = (choices::level_entries(tier).iter())
            .map(|item| (item.title.to_string(), item.enabled, item.checked))
            .collect();
        expected.extend([
            (String::new(), false, false),
            ("Settings…".into(), true, false),
        ]);
        assert_eq!(rows, expected, "the level menu's rows at {tier:?}");
        assert_eq!(
            view.read_with(cx, |view, _| view.menu_open),
            Some("conversation-level"),
            "the trigger wears its plate while its menu is open"
        );
        host.take();
        let (pick, pressed) = match tier {
            Tier::Full => (expected.len() - 1, "agent access"),
            _ => (1, "task task-1 PickTier(Work)"),
        };
        cx.update(|window, cx| chosen(crate::menu::Closed::Chose(pick), window, cx));
        cx.run_until_parked();
        assert_eq!(
            host.take(),
            vec![pressed.to_string()],
            "the pick is the user's press"
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.menu_open),
            None,
            "its plate went"
        );
    }
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
        started: None,
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
        started: None,
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

/// A stretch's fold says Z14's words, the one place the weaving pair stands: at work its word and
/// the time so far from its first step, done the time it took. It counts no steps or strands and
/// has no dot, whatever the card says. A step's line has no dot before its note.
#[test]
fn a_stretchs_fold_says_weaving_then_woven_in() {
    use super::{TraceLine, summary_line};
    for helpers in [0, 1, 4] {
        let mut talk = with_helpers(helpers);
        let trace = its_trace(&mut talk);
        let now = std::time::Instant::now();
        trace.started = Some(now - std::time::Duration::from_secs(72));
        assert_eq!(summary_line(trace, now).text(), "Weaving 1 min 12 s");
        (trace.running, trace.took) = (false, Some(41));
        let drawn = summary_line(trace, now).text();
        assert_eq!(drawn, "Woven in 41 s");
        assert!(!drawn.contains('\u{b7}'), "{drawn}");
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
    assert_eq!(summary(&mut with_helpers(4)), "Working · 4 strands");
    assert_eq!(summary(&mut with_helpers(1)), "Working · 1 strand");
    assert_eq!(summary(&mut with_helpers(0)), "Working · 1 step");

    // One of the four waits on the user and one has finished: each still has its line, so each
    // still counts.
    let mut talk = with_helpers(4);
    its_trace(&mut talk).items[1] = helper(step(11, "Read shop.example", false));
    its_trace(&mut talk).items[2] = helper(step(12, "Handed you shop.example", false));
    assert_eq!(summary(&mut talk), "Working · 4 strands");

    let over = its_trace(&mut talk);
    (over.running, over.took) = (false, Some(41));
    assert_eq!(summary(&mut talk), "Worked for 41 s · 4 strands");
}

/// Opened, as it is while it works, the Working line lists a line for each helper under the line
/// that says the work split, with the task's own steps, and folds them away with a press, as any
/// trace's.
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
        Some(lines_high(6.)),
        "the split's line, four helpers' lines and the task's own step"
    );

    // A fifth line as a helper comes, and none goes as one finishes.
    let mut talk = with_helpers(5);
    its_trace(&mut talk).items[0] = helper(step(10, "Read shop.example", false));
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    draw(cx);
    assert_eq!(listed(cx), Some(lines_high(7.)));

    view.update(cx, |view, cx| view.toggle(Fold::Trace(0), true, cx));
    draw(cx);
    assert_eq!(listed(cx), None, "folded, it still listed them");
}

/// The stage shows no page, so no page's row stands over its composer: the step under way is the
/// log's last line, and a tile is the way to a page and its Take Over. Whether the thread works on
/// a page or the user took it over, the stage draws neither Take Over nor Hand Back there.
#[gpui::test]
fn the_stage_has_no_page_row_over_its_composer(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let yours = Live::Yours {
        tab: 30,
        site: None,
    };
    for live in [Live::Working, yours] {
        let talk = Conversation {
            live: live.clone(),
            ..conversation(3)
        };
        assert!(super::under_way(&talk.lines).is_some(), "no step under way");
        assert_eq!(talk.on, Some(30), "the thread isn't on a page");
        shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
        view.update(cx, |view, cx| {
            view.take_over_drawn.borrow_mut().clear();
            view.hand_back_drawn.set(None);
            cx.notify();
        });
        cx.run_until_parked();
        let chips = view.read_with(cx, |view, _| view.take_over_drawn.borrow().len());
        assert_eq!(
            chips, 0,
            "Take Over stood over the stage's composer at {live:?}"
        );
        let hand_back = view.read_with(cx, |view, _| view.hand_back_drawn.get());
        assert_eq!(
            hand_back, None,
            "Hand Back stood over the stage's composer at {live:?}"
        );
    }
}

/// The panel's page row names the step under way only: once that step has ended, as at Hand
/// Back, or was refused, none is; while the task waits on the user it says nothing, and on a page
/// the user took over that they're driving.
#[test]
fn the_page_row_names_only_the_step_under_way() {
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
    let yours = |site: Option<&str>| Live::Yours {
        tab: 30,
        site: site.map(Into::into),
    };
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
}

/// Take Over is drawn once, in the panel's page row, for the task's own step under way on the page
/// it takes, never on a line: a helper's line under way has none, as its page is another.
#[gpui::test]
fn a_helpers_line_has_no_take_over(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(with_helpers(4)), cx);
    });
    watch_page(&shell, cx);
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
    let (shell, _, cx) = shown_with(card.clone(), cx);
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

/// While a stretch works, the page the assistant is on stands open with its steps and a page it
/// left is one line; a click on the left page brings its steps back and leaves the other as it
/// was. Going to a page has no line of its own under it.
#[gpui::test]
fn a_left_page_folds_and_opens_alone(cx: &mut TestAppContext) {
    use crate::agents::task::OnPage;
    let (shell, _, cx) = shown(cx);
    let on = |key: usize, tab: Option<u64>, site: &str, went: bool, running: bool| {
        TraceItem::Step(TraceStep {
            page: Some(OnPage {
                tab,
                site: site.to_string().into(),
                title: "".into(),
                went,
            }),
            ..step(key, &format!("step {key}"), running)
        })
    };
    let mut talk = conversation(1);
    talk.lines = vec![Line::Trace(Trace {
        key: 0,
        running: true,
        took: None,
        started: None,
        items: vec![
            on(0, None, "a.example", true, false),
            on(1, Some(30), "a.example", false, false),
            on(2, Some(30), "a.example", false, false),
            on(3, Some(30), "b.example", true, false),
            on(4, Some(30), "b.example", false, true),
        ],
    })];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let draw = |cx: &mut VisualTestContext| {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    };
    let height = |fold: Fold, current: bool, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            (view.is_open(fold, current)).then(|| view.heights.borrow().get(&fold).copied())
        })
    };
    draw(cx);
    assert_eq!(
        height(Fold::Page(3), true, cx),
        Some(Some(lines_high(1.))),
        "the page it's on isn't open to its one step, without the step that went to it"
    );
    assert_eq!(
        height(Fold::Page(0), false, cx),
        None,
        "the left page stood open"
    );

    view.update(cx, |view, cx| view.toggle(Fold::Page(0), false, cx));
    draw(cx);
    assert_eq!(
        height(Fold::Page(0), false, cx),
        Some(Some(lines_high(2.))),
        "the left page didn't bring back its two steps"
    );
    assert!(
        height(Fold::Page(3), true, cx).is_some(),
        "opening the left page folded the one it's on"
    );
}

/// The button under an answer copies the answer's plain text, its marks taken out, and the time it
/// finished stands beside it.
#[gpui::test]
fn the_copy_button_copies_the_answers_plain_text(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let mut talk = conversation(1);
    talk.lines.push(Line::Said(
        "**Two** shows at [MAAT](https://maat.pt)".into(),
    ));
    talk.finished = vec![(1, "18:00".into())];
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    view.update(cx, |view, cx| view.copy_answer(1, cx));
    let copied = cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(
        copied.as_deref(),
        Some("Two shows at MAAT (https://maat.pt)")
    );
}

/// Copying an answer keeps every link's address after its words, once, and a bare address alone.
#[test]
fn a_copied_answer_keeps_its_links_addresses() {
    let copied = super::plain(
        "See [the agenda](https://ccb.pt/agenda) and https://maat.pt\n- [MAAT](https://maat.pt)",
    );
    assert_eq!(
        copied,
        "See the agenda (https://ccb.pt/agenda) and https://maat.pt\n\u{2022} MAAT (https://maat.pt)"
    );
}

/// An answer's link to one of the thread's pages shows that page; one to anywhere else on the web
/// opens a tab of the user's Space, as a link always has; any other address is plain words.
#[test]
fn an_answers_link_shows_its_page_or_opens_the_web() {
    use super::Linked;
    let pages = [(30, "https://maat.pt/".into())];
    let to = |url: &str| super::linked_to(&pages, &url.into());
    assert_eq!(to("https://maat.pt"), Some(Linked::Page(30)));
    assert_eq!(
        to("https://elsewhere.example/a"),
        Some(Linked::Web("https://elsewhere.example/a".into()))
    );
    assert_eq!(
        to("http://elsewhere.example/"),
        Some(Linked::Web("http://elsewhere.example/".into()))
    );
    assert_eq!(to("file:///etc/hosts"), None);
    assert_eq!(to("javascript:alert(1)"), None);
}

/// A web link's click asks the core for a tab of the user's Space and nothing of the thread. No
/// log line says the address.
#[gpui::test]
fn a_web_links_click_opens_a_tab_of_the_users_space(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(1)), cx)
    });
    cx.run_until_parked();
    host.take();
    shell.update(cx, |shell, cx| {
        shell.open_link("https://elsewhere.example/a", cx)
    });
    assert_eq!(
        host.opened_pages.borrow().as_slice(),
        ["https://elsewhere.example/a"]
    );
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.starts_with("task")),
        "the thread was asked something: {calls:?}"
    );
}

/// Where a link's words are themselves an address on another host than the link's, the link's
/// real address is drawn in the words' place, and copied so; words on the same host, or not an
/// address, stay.
#[test]
fn a_link_whose_words_name_another_host_draws_its_real_address() {
    let drawn = |source: &str| match super::markdown::blocks(source).remove(0) {
        super::markdown::Block::Paragraph(text) => super::honest(text),
        other => panic!("not a paragraph: {other:?}"),
    };
    let swapped = drawn("Pay at **[bank.example](https://evil.example/pay)** now");
    assert_eq!(swapped.text, "Pay at https://evil.example/pay now");
    assert_eq!(swapped.links.len(), 1);
    let (range, url) = &swapped.links[0];
    assert_eq!(&swapped.text[range.clone()], url.as_ref());
    assert!(
        swapped
            .styled
            .iter()
            .any(|(each, _)| &swapped.text[each.clone()] == url.as_ref())
    );
    let same = drawn("[www.maat.pt](https://maat.pt/agenda)");
    assert_eq!(same.text, "www.maat.pt");
    let words = drawn("[the agenda](https://ccb.pt/agenda)");
    assert_eq!(words.text, "the agenda");
    assert_eq!(
        super::plain("[bank.example](https://evil.example/pay)"),
        "https://evil.example/pay"
    );
}

/// An address with a `user:pass@` part opens and is drawn without it: the link's address loses
/// it, words that hold it are drawn as the address without it, and Copy keeps neither.
#[test]
fn a_links_sign_in_part_is_never_opened_or_drawn() {
    let drawn = |source: &str| match super::markdown::blocks(source).remove(0) {
        super::markdown::Block::Paragraph(text) => super::honest(text),
        other => panic!("not a paragraph: {other:?}"),
    };
    let words = drawn("[the shop](https://me:hunter2@shop.example/a)");
    assert_eq!(words.text, "the shop");
    assert_eq!(words.links[0].1.as_ref(), "https://shop.example/a");
    let pages = [(30, "https://shop.example/b".into())];
    assert!(matches!(
        super::linked_to(&pages, &words.links[0].1),
        Some(super::Linked::Web(url)) if url.as_ref() == "https://shop.example/a"
    ));
    let address = drawn("[https://me:hunter2@shop.example/a](https://me:hunter2@shop.example/a)");
    assert_eq!(address.text, "https://shop.example/a");
    let (range, url) = &address.links[0];
    assert_eq!(&address.text[range.clone()], url.as_ref());
    let copied = super::plain("See [the shop](https://me:hunter2@shop.example/a).");
    assert!(
        !copied.contains("hunter2") && !copied.contains("me:"),
        "{copied}"
    );
    let (host, rest) = super::link_tip("https://me:hunter2@shop.example/a").unwrap();
    assert!(!format!("{host}{rest}").contains("hunter2"));
}

/// A web link's tooltip says its host, then the rest of its address, each as plain text cut in the
/// middle, a host in capitals as any.
#[test]
fn a_web_links_tooltip_is_its_address_plain_and_cut() {
    let (host, rest) = super::link_tip("https://maat.pt/agenda?day=sat").unwrap();
    assert_eq!(
        (host.as_ref(), rest.as_ref()),
        ("maat.pt", "/agenda?day=sat")
    );
    let long = format!("https://maat.pt/{}", "a".repeat(30) + &"z".repeat(30));
    let (_, rest) = super::link_tip(&long).unwrap();
    assert_eq!(rest.chars().count(), 48);
    assert!(
        rest.starts_with("/aaa") && rest.ends_with("zzz") && rest.contains('\u{2026}'),
        "{rest}"
    );
    // A host in capitals has its tooltip too, the host as it is found.
    let (host, rest) = super::link_tip("https://MAAT.PT/Agenda").unwrap();
    assert_eq!((host.as_ref(), rest.as_ref()), ("maat.pt", "/Agenda"));
    // A host past the cap is cut as the rest is.
    let long = format!("https://{}.example/", "h".repeat(80));
    let (host, _) = super::link_tip(&long).unwrap();
    assert_eq!(host.chars().count(), 48);
    assert!(
        host.ends_with("h.example") && host.contains('\u{2026}'),
        "{host}"
    );
}

/// An answer's link is one only to a page of its thread, the same address but for a closing slash
/// or fragment; an address anywhere else isn't a link.
#[test]
fn the_answers_links_go_only_to_the_threads_pages() {
    let pages = [
        (30, "https://maat.pt/".into()),
        (31, "https://ccb.pt/agenda".into()),
    ];
    assert_eq!(super::thread_tab(&pages, "https://maat.pt"), Some(30));
    assert_eq!(
        super::thread_tab(&pages, "https://ccb.pt/agenda#today"),
        Some(31)
    );
    assert_eq!(super::thread_tab(&pages, "https://ccb.pt/"), None);
    assert_eq!(
        super::thread_tab(&pages, "https://elsewhere.example/"),
        None
    );
}

/// Standing on the page the thread works on, the panel's row over the reply field has the step
/// under way with Take Over, which takes that page; on a page the user took over it says so with
/// Hand Back; with the agent on no page of the user's, or nothing under way, there's no row.
#[gpui::test]
fn the_panels_page_row_takes_over_the_page_shown(cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(conversation(3)), cx);
    });
    watch_page(&shell, cx);
    let drawn = |cx: &mut VisualTestContext| {
        view.update(cx, |view, cx| {
            view.take_over_drawn.borrow_mut().clear();
            view.hand_back_drawn.set(None);
            cx.notify();
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            (
                view.take_over_drawn.borrow().clone(),
                view.hand_back_drawn.get(),
            )
        })
    };
    let (take_over, hand_back) = drawn(cx);
    assert_eq!(take_over.len(), 1, "no Take Over in the page's row");
    assert_eq!(hand_back, None);
    host.take();
    cx.simulate_click(take_over[0].center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 TakeOver(30)"]);

    let mut taken = conversation(3);
    taken.live = Live::Yours {
        tab: 30,
        site: None,
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(taken), cx));
    let (take_over, hand_back) = drawn(cx);
    assert!(take_over.is_empty(), "Take Over on a page the user has");
    let hand_back = hand_back.expect("no Hand Back in the page's row");
    host.take();
    cx.simulate_click(hand_back.center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 HandBack(30)"]);

    let mut elsewhere = conversation(3);
    elsewhere.on = None;
    shell.update(cx, |shell, cx| shell.set_conversation(Some(elsewhere), cx));
    assert_eq!(
        drawn(cx),
        (Vec::new(), None),
        "a row for a page it isn't on"
    );
    let mut done = conversation(3);
    done.stoppable = false;
    shell.update(cx, |shell, cx| shell.set_conversation(Some(done), cx));
    assert_eq!(
        drawn(cx),
        (Vec::new(), None),
        "a row with nothing under way"
    );
}

/// Where the page row's Take Over and Hand Back were drawn last, after the panel draws again.
fn row_buttons_drawn(
    view: &Entity<super::TalkView>,
    cx: &mut VisualTestContext,
) -> (
    Vec<gpui::Bounds<gpui::Pixels>>,
    Option<gpui::Bounds<gpui::Pixels>>,
) {
    view.update(cx, |view, cx| {
        view.take_over_drawn.borrow_mut().clear();
        view.hand_back_drawn.set(None);
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        (
            view.take_over_drawn.borrow().clone(),
            view.hand_back_drawn.get(),
        )
    })
}

/// Standing on page 30, which the user took over, while the thread's line is `live` about another
/// of its pages (31, the one it's `on` if any): the row is theirs, with the line's words for a
/// page held, Hand Back is drawn and Take Over isn't, and the press hands that page back by the
/// strip's own road.
fn yours_on_a_held_page(live: Live, on: Option<u64>, cx: &mut TestAppContext) {
    let (shell, host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let tabs: Vec<(u64, gpui::SharedString)> =
        vec![(30, "skyscanner.net".into()), (31, "maat.pt".into())];
    let split = Conversation {
        tabs: tabs.clone(),
        on,
        held: vec![30],
        live: live.clone(),
        ..conversation(3)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(split), cx));
    watch_page(&shell, cx);
    let row = view.read_with(cx, |view, _| view.page_row_for(30, None));
    let yours = crate::agents::task::yours(30, &tabs);
    let said = super::words::live(&yours).expect("words for a page the user has");
    assert_eq!(
        row.case,
        Some(super::RowCase::Yours(said.into())),
        "{live:?}"
    );
    let (take_over, hand_back) = row_buttons_drawn(&view, cx);
    assert!(
        take_over.is_empty(),
        "Take Over on a page the user has: {live:?}"
    );
    let hand_back = hand_back.expect("no Hand Back in the page's row");
    host.take();
    cx.simulate_click(hand_back.center(), Modifiers::none());
    assert_eq!(host.take(), ["task task-1 HandBack(30)"], "{live:?}");
    let stop = shell.read_with(cx, |shell, cx| shell.page_row_shown(30, cx));
    let stop = stop.case.map(|case| case.button());
    assert_eq!(stop, Some(crate::shell::page_ends::StripStop::HandBack));
}

/// A page the user took over is theirs on its row while the thread works on another.
#[gpui::test]
fn the_panels_page_row_is_yours_on_a_page_held_while_the_thread_works_on_another(
    cx: &mut TestAppContext,
) {
    yours_on_a_held_page(Live::Working, Some(31), cx);
}

/// A page the user took over is theirs on its row while another page's hand-off waits.
#[gpui::test]
fn the_panels_page_row_is_yours_on_a_page_held_while_another_is_handed(cx: &mut TestAppContext) {
    yours_on_a_held_page(Live::Handed, Some(31), cx);
}

/// A page the user took over is theirs on its row while the thread's question waits.
#[gpui::test]
fn the_panels_page_row_is_yours_on_a_page_held_while_a_question_waits(cx: &mut TestAppContext) {
    yours_on_a_held_page(Live::Asked, None, cx);
}

/// A page the user took over is theirs on its row while a refused step on another waits.
#[gpui::test]
fn the_panels_page_row_is_yours_on_a_page_held_while_a_refusal_waits(cx: &mut TestAppContext) {
    yours_on_a_held_page(Live::Refused, Some(31), cx);
}

/// Standing on the page an agent handed the user, the hand-off's own line has its Hand Back and
/// the row draws none: it isn't a page the user took over.
#[gpui::test]
fn a_hand_offs_own_page_draws_no_second_hand_back(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let handed = Conversation {
        tabs: vec![(30, "skyscanner.net".into()), (31, "maat.pt".into())],
        on: Some(30),
        held: Vec::new(),
        live: Live::Handed,
        ..conversation(3)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(handed), cx));
    watch_page(&shell, cx);
    let row = view.read_with(cx, |view, _| view.page_row_for(30, None));
    assert!(
        !matches!(row.case, Some(super::RowCase::Yours(_))),
        "{:?}",
        row.case
    );
    assert_eq!(row_buttons_drawn(&view, cx).1, None, "a second Hand Back");
}

/// Standing on a page the user doesn't hold, while they hold another of the thread's, the row
/// isn't theirs: no Hand Back for the page stood on.
#[gpui::test]
fn the_panels_page_row_isnt_yours_on_a_page_not_held(cx: &mut TestAppContext) {
    let (shell, _host, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let tabs: Vec<(u64, gpui::SharedString)> = vec![
        (30, "skyscanner.net".into()),
        (31, "maat.pt".into()),
        (32, "ccb.pt".into()),
    ];
    let working = Conversation {
        tabs: tabs.clone(),
        on: Some(32),
        held: vec![31],
        ..conversation(3)
    };
    let theirs = Conversation {
        tabs,
        on: None,
        held: vec![31],
        live: Live::Yours {
            tab: 31,
            site: Some("maat.pt".into()),
        },
        ..conversation(3)
    };
    for talk in [working, theirs] {
        shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
        watch_page(&shell, cx);
        let row = view.read_with(cx, |view, _| view.page_row_for(30, None));
        assert!(
            !matches!(row.case, Some(super::RowCase::Yours(_))),
            "{:?}",
            row.case
        );
        assert_eq!(
            row_buttons_drawn(&view, cx).1,
            None,
            "Hand Back on a page not held"
        );
    }
}

/// A thread's `primary`, under the round Stop and the held step's first button, is the same in
/// every Space's colour, one per look, its words readable on it: never the Space's accent, so no
/// control is green.
#[test]
fn a_threads_primary_is_never_the_spaces_colour() {
    use crate::palette::{Palette, Tone, contrast};
    use crate::theme::{Appearance, Theme};
    for tone in [Tone::Dark, Tone::Light] {
        let first = super::primary(Palette::of(None, tone));
        for appearance in Appearance::ALL {
            for (x, y) in [(0.7, 0.), (-0.7, 0.), (0., 0.7), (0., -0.7), (0.5, 0.5)] {
                let theme = Theme {
                    appearance,
                    ..Theme::at(x, y)
                };
                let palette = Palette::of(Some(&theme), tone);
                let (fill, ink) = super::primary(palette);
                assert_eq!((fill, ink), first, "{tone:?} {x},{y}");
                assert_ne!(fill, gpui::Hsla::from(palette.accent), "{tone:?} {x},{y}");
                let ratio = contrast(gpui::Rgba::from(ink), gpui::Rgba::from(fill));
                assert!(ratio >= 4.5, "{ratio:.2}:1");
            }
        }
    }
}

/// What came of a step sits at the column's right edge, its words filling the line between; a
/// fold's head, a page's line and a line read as one string keep theirs after their words.
#[test]
fn what_came_of_a_step_sits_at_the_columns_right() {
    use super::TraceLine;
    let line = |fold| TraceLine::new(fold, "Opened tickets.example".into(), "2 s".into());
    assert!(line(Fold::Step(1)).note_at_right());
    let mut head = line(Fold::Trace(0));
    head.head = Some(true);
    let mut page = line(Fold::Page(2));
    page.page = Some(false);
    let mut joined = line(Fold::Trace(0));
    joined.joined = true;
    for kept in [head, page, joined] {
        assert!(!kept.note_at_right());
    }
}

/// The assistant's words said between steps are in the text's ink, as the answer's are, and read
/// on the stage in either look: never the quiet ink a thought and a step's past are in.
#[test]
fn said_words_between_steps_are_in_the_texts_ink() {
    use crate::palette::{Palette, Tone, contrast, over};
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        let ink = super::said_ink(&palette);
        assert_ne!(ink, palette.gray(crate::shell::SECONDARY), "{tone:?}");
        let quiet = crate::tokens::shell::conversation::TRACE_NOTE;
        assert_ne!(ink, palette.gray(quiet), "{tone:?}");
        let ground = gpui::Rgba {
            a: 1.,
            ..super::stage_fill(&palette)
        };
        let ink = gpui::Rgba::from(ink);
        let ratio = contrast(over(ground, gpui::Rgba { a: 1., ..ink }, ink.a), ground);
        assert!(ratio >= 7., "{tone:?} {ratio:.2}:1");
    }
}

/// A failed line draws no colour that isn't the text's or text-2's, in either look: its words,
/// its why and its glyph are the inks of the same line had it gone through, at rest and under the
/// pointer, and each is the text's ink at some strength. The find bar's red is the find bar's.
#[test]
fn a_failed_line_wears_no_red() {
    use super::TraceLine;
    use crate::palette::{Palette, Tone};
    use crate::tokens::shell::conversation::TRACE_NOTE;
    let step = || {
        let why = "There's no element e7 on the page.";
        let mut line = TraceLine::new(Fold::Step(1), "Couldn't click “Pay”".into(), why.into());
        line.glyph = Some("icons/x.svg");
        line
    };
    // One that opens to why, one that doesn't, and a turn that couldn't start.
    let opens = || TraceLine {
        chevron: Some(0.),
        ..step()
    };
    let unstarted = || TraceLine::new(Fold::Trace(0), "Couldn't start".into(), "".into());
    for tone in [Tone::Dark, Tone::Light] {
        let palette = Palette::of(None, tone);
        let ink = gpui::Hsla::from(palette.ink);
        let lines: [&dyn Fn() -> TraceLine; 3] = [&step, &opens, &unstarted];
        for line in lines {
            let failed = TraceLine {
                failed: true,
                ..line()
            };
            let inks = failed.inks(&palette);
            assert_eq!(
                inks,
                line().inks(&palette),
                "{tone:?}: a failed line's own ink"
            );
            assert_eq!(
                inks.note,
                palette.gray(TRACE_NOTE),
                "{tone:?}: why in text-2"
            );
            let drawn = [
                Some(inks.words),
                Some(inks.note),
                Some(inks.glyph),
                inks.pointed,
            ];
            for colour in drawn.into_iter().flatten() {
                let opaque = gpui::Hsla { a: ink.a, ..colour };
                assert_eq!(opaque, ink, "{tone:?}: a colour that isn't the text's");
            }
        }
        assert!(
            (TraceLine {
                failed: true,
                ..opens()
            })
            .inks(&palette)
            .pointed
            .is_some(),
            "{tone:?}: under the pointer it lights as any line that opens"
        );
    }
}

/// The user's message wraps inside the panel's column beside a page, never wider than it, so none
/// of it leaves the panel; the stage's measure wouldn't fit there.
#[test]
fn a_users_message_in_the_panel_fits_its_column() {
    use crate::tokens::shell::conversation::panel::BODY_X;
    use crate::tokens::shell::task_area::BESIDE;
    let column = BESIDE - 2. * BODY_X;
    let (panel, stage) = (super::Bubble::of(true), super::Bubble::of(false));
    assert!(
        panel.most <= column,
        "{panel:?} is wider than the panel's {column}"
    );
    assert!(
        stage.most > column,
        "the stage's {stage:?} would fit the panel"
    );
    assert!(panel.text < stage.text && panel.line < stage.line);
}

/// Task-1's card `card` and conversation `talk`, standing on tab 30 with the panel beside it.
fn standing_with(
    card: Card,
    talk: Conversation,
    cx: &mut TestAppContext,
) -> (
    Entity<Shell>,
    Rc<Recorder>,
    Entity<super::TalkView>,
    &mut VisualTestContext,
) {
    let (shell, host, cx) = shown_with(card, cx);
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    watch_page(&shell, cx);
    // The core shows the task's Space and its page, as once the page is in place.
    shell.update(cx, |shell, cx| {
        shell.set_space(3, cx);
        shell.set_selected(30, cx);
    });
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    redraw(&shell, &view, cx);
    assert!(shell.read_with(cx, |shell, _| shell.task_standing()));
    (shell, host, view, cx)
}

/// Standing on one of the thread's pages, the panel head's button of its other page stands on that
/// one at once. Its press runs in the panel's own update, as here, so nothing the shell does for it
/// may read the panel.
#[gpui::test]
fn the_panel_heads_button_of_its_other_page_stands_on_it(cx: &mut TestAppContext) {
    let talk = Conversation {
        tabs: vec![(30, "skyscanner.net".into()), (31, "maat.pt".into())],
        ..conversation(3)
    };
    let (shell, host, view, cx) = standing_with(card(), talk, cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            3,
            Vec::new(),
            vec![30, 31],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    redraw(&shell, &view, cx);
    host.take();
    // As the button's listener does: the stand runs inside the panel's own update.
    view.update(cx, |view, cx| view.stand(31, cx));
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.task_shown()),
        Some((3, Some(31)))
    );
    assert!(host.take().contains(&"select 31".to_string()));
}

fn redraw(shell: &Entity<Shell>, view: &Entity<super::TalkView>, cx: &mut VisualTestContext) {
    view.update(cx, |_, cx| cx.notify());
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// The ring's stops once round from the window's Tab, the task's header left out, each checked as
/// drawn where it rings: a page the user holds is gone into and left again, as the user's is.
fn ring_round(
    shell: &Entity<Shell>,
    view: &Entity<super::TalkView>,
    host: &Recorder,
    cx: &mut VisualTestContext,
) -> Vec<String> {
    use crate::shell::page_ends::STOPS_DRAWN;
    assert!(
        shell.update(cx, |shell, cx| shell.tab_into_strip(false, cx)),
        "Tab from the window rang nothing"
    );
    let mut rung = Vec::new();
    for _ in 0..8 {
        STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
        redraw(shell, view, cx);
        let Some(stop) = shell.read_with(cx, |shell, _| shell.keyed_strip) else {
            panic!("the ring left the page's stops after {rung:?}");
        };
        let name = format!("{stop:?}");
        if rung.contains(&name) {
            break;
        }
        assert!(
            STOPS_DRAWN.with(|drawn| drawn.borrow().contains_key(&stop)),
            "the ring is on {stop:?}, which isn't drawn"
        );
        rung.push(name);
        host.take();
        shell.update(cx, |shell, cx| shell.key_tab(false, cx));
        if shell.read_with(cx, |shell, _| shell.keyed_strip).is_none() {
            assert_eq!(host.take(), ["focus page first"], "after {stop:?}");
            rung.push("into the page".into());
            // Later than the press that went in, which a page with nothing to focus hands back.
            assert!(shell.update(cx, |shell, cx| {
                shell.page_ends.entered = None;
                shell.tab_focus_leaving(30, false, cx)
            }));
        }
    }
    rung.retain(|stop| !stop.starts_with("Header"));
    rung
}

/// The page row in each of its cases, and an outside app's panel, which has no composer: what the
/// ring goes round, and that Return on the row's button does what a click on it does. The thread
/// works on the page shown: its words and Take Over. The user holds it, taken over or handed to
/// them: the page itself, and Hand Back where the row has it. The thread ended with the page to
/// move: Open alone, with ilo's line in the field's place too. An outside app's session: its
/// words, Take Over and Stop, or Hand Back and Stop on a page its user holds. Waiting on the user
/// for a refused step or its question, at work on another page or between steps, the row holds
/// nothing and the page is the agent's, which the ring never enters.
#[gpui::test]
fn the_page_row_answers_the_pointer_and_the_ring_alike_in_each_case(cx: &mut TestAppContext) {
    use crate::shell::page_ends::STOPS_DRAWN;
    let (session, session_talk) = session_panel();
    for case in row_cases() {
        let name = case.name;
        // Only a held page's status changes the row; the agent at work is left out, as before.
        let status = case.status.clone().filter(|status| !status.held.is_empty());
        let calls = |pressed: bool, cx: &mut TestAppContext| {
            let (shell, host, view, cx) = standing_with(case.card.clone(), case.talk.clone(), cx);
            if let Some(status) = status.clone() {
                shell.update(cx, |shell, cx| shell.set_agent_status(vec![status], cx));
                redraw(&shell, &view, cx);
            }
            let went = ring_round(&shell, &view, &host, cx);
            assert_eq!(went, case.ring, "{name}: the ring's round");
            let button = case.button?;
            shell.update(cx, |shell, cx| shell.leave_strip("test", cx));
            STOPS_DRAWN.with(|drawn| drawn.borrow_mut().clear());
            redraw(&shell, &view, cx);
            let at = STOPS_DRAWN.with(|drawn| drawn.borrow().get(&button).copied());
            let at = at.unwrap_or_else(|| panic!("{name}: {button:?} isn't drawn"));
            host.take();
            match pressed {
                true => shell.update(cx, |shell, cx| shell.key_strip_show(button, cx)),
                false => cx.simulate_click(at.center(), Modifiers::none()),
            }
            cx.run_until_parked();
            // A click moves the pointer, which takes down a tooltip it rested under, such as an
            // outside app's line's in the sidebar; a key doesn't.
            let calls: Vec<String> = (host.take().into_iter())
                .filter(|call| {
                    !call.starts_with("focus")
                        && !call.starts_with("picture")
                        && !call.starts_with("tooltip")
                })
                .collect();
            assert!(!calls.is_empty(), "{name}: {button:?} did nothing");
            Some(calls)
        };
        assert_eq!(
            calls(true, cx),
            calls(false, cx),
            "{name}: Return on {:?} didn't do what its click does",
            case.button
        );
    }

    // An outside app's panel has no composer, where a task's has one.
    let (_, _, view, _) = standing_with(session, session_talk, cx);
    assert_eq!(view.read_with(cx, |view, _| view.reply_drawn.get()), None);
    let (_, _, view, _) = standing_with(card(), conversation(3), cx);
    assert!(
        view.read_with(cx, |view, _| view.reply_drawn.get())
            .is_some()
    );
}

/// One of the page row's cases, standing on tab 30.
#[derive(Clone)]
struct RowCaseShown {
    name: &'static str,
    card: Card,
    talk: Conversation,
    /// What the agents say of it.
    status: Option<Status>,
    /// The ring's stops once round, the task's header left out.
    ring: Vec<&'static str>,
    /// The row's last button, if it has one.
    button: Option<crate::shell::page_ends::StripStop>,
    /// What ⌘↩ from the page asks of the core: Take Over or Hand Back, where the case has one.
    command: Option<&'static str>,
    /// Whether ⌘. from the page stops the work.
    stops: bool,
}

/// The page row's cases, each standing on tab 30, and an outside app's panels.
fn row_cases() -> Vec<RowCaseShown> {
    use crate::shell::page_ends::StripStop;
    let at_work = |agent: &str, name: &str| Status {
        agent: agent.into(),
        name: name.into(),
        space: 3,
        doing: "Searching flights".into(),
        held: Vec::new(),
    };
    let held = |agent: &str, name: &str, why: &str| Status {
        held: vec![(30, why.into())],
        doing: "".into(),
        ..at_work(agent, name)
    };
    let live = |live: Live| Conversation {
        live,
        ..conversation(3)
    };
    let yours = Live::Yours {
        tab: 30,
        site: None,
    };
    let waits = Card {
        dot: Dot::Waiting,
        wait: Default::default(),
        word: Word::for_dot(Dot::Waiting),
        ..card()
    };
    let done = Card {
        dot: Dot::Done,
        wait: Default::default(),
        word: Word::for_dot(Dot::Done),
        open: vec![(30, "skyscanner.net".into())],
        ..card()
    };
    let ended = Conversation {
        stoppable: false,
        ..conversation(3)
    };
    // The task has gone: ilo's line says so in the field's place.
    let gone = Conversation {
        lines: vec![Line::Ilo(IloLine {
            key: 4,
            kind: IloKind::Gone,
            settled: false,
        })],
        ..ended.clone()
    };
    let between = Conversation {
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            started: None,
            items: vec![TraceItem::Step(step(0, "Opened skyscanner.net", false))],
        })],
        ..conversation(1)
    };
    let elsewhere = Conversation {
        tabs: vec![(30, "skyscanner.net".into()), (31, "maat.pt".into())],
        on: Some(31),
        ..conversation(3)
    };
    let (session, session_talk) = session_panel();
    let session_held = Conversation {
        live: yours.clone(),
        ..session_talk.clone()
    };
    let case = |name, card: &Card, talk: Conversation, status: Option<Status>| RowCaseShown {
        name,
        card: card.clone(),
        talk,
        status,
        ring: Vec::new(),
        button: None,
        command: None,
        stops: true,
    };
    let task_take_over = Some("take over task-1 30");
    vec![
        RowCaseShown {
            ring: vec!["Page", "TakeOver"],
            button: Some(StripStop::TakeOver),
            command: task_take_over,
            ..case(
                "working",
                &card(),
                conversation(3),
                Some(at_work("task-1", "Claude")),
            )
        },
        RowCaseShown {
            ring: vec!["into the page", "HandBack"],
            button: Some(StripStop::HandBack),
            command: Some("hand back 30"),
            ..case(
                "yours",
                &card(),
                live(yours.clone()),
                Some(held("task-1", "Claude", "")),
            )
        },
        RowCaseShown {
            ring: vec!["into the page"],
            command: Some("hand back 30"),
            ..case(
                "handed",
                &waits,
                live(Live::Handed),
                Some(held("task-1", "Claude", "Sign in")),
            )
        },
        RowCaseShown {
            command: task_take_over,
            ..case(
                "refused",
                &waits,
                live(Live::Refused),
                Some(at_work("task-1", "Claude")),
            )
        },
        RowCaseShown {
            command: task_take_over,
            ..case(
                "asked",
                &waits,
                live(Live::Asked),
                Some(at_work("task-1", "Claude")),
            )
        },
        RowCaseShown {
            command: task_take_over,
            ..case(
                "elsewhere",
                &card(),
                elsewhere,
                Some(at_work("task-1", "Claude")),
            )
        },
        RowCaseShown {
            command: task_take_over,
            ..case(
                "between steps",
                &card(),
                between,
                Some(at_work("task-1", "Claude")),
            )
        },
        RowCaseShown {
            ring: vec!["Open"],
            button: Some(StripStop::Open),
            stops: false,
            ..case("ended", &done, ended, None)
        },
        RowCaseShown {
            ring: vec!["Open"],
            button: Some(StripStop::Open),
            stops: false,
            ..case("gone", &done, gone, None)
        },
        RowCaseShown {
            ring: vec!["Page", "TakeOver", "Stop"],
            button: Some(StripStop::Stop),
            command: Some("take over session-1 30"),
            ..case(
                "session",
                &session,
                session_talk,
                Some(at_work("session-1", "Claude Code")),
            )
        },
        RowCaseShown {
            ring: vec!["into the page", "HandBack", "Stop"],
            button: Some(StripStop::HandBack),
            command: Some("hand back 30"),
            ..case(
                "session held",
                &session,
                session_held,
                Some(held("session-1", "Claude Code", "")),
            )
        },
    ]
}

/// An outside app's panel has its card's line under the title, from the kind of its connection
/// alone, and no reply field; a task's panel never has it, whatever either is named. However long
/// the name, or made of the product's own state words, the line is one line where it was and the
/// page row and Stop are as they were.
#[gpui::test]
fn an_outside_apps_panel_has_its_line_under_the_title(cx: &mut TestAppContext) {
    use crate::tokens::shell::task_card::FROM_LINE;
    let named = |name: &str| {
        let (session, talk) = session_panel();
        let session = Card {
            name: name.into(),
            ..session
        };
        let talk = Conversation {
            name: name.into(),
            ..talk
        };
        (session, talk)
    };
    let (session, talk) = named("Claude Code");
    let (_, _, view, cx) = standing_with(session, talk, cx);
    let line = view.read_with(cx, |view, _| view.from_drawn.get());
    let line = line.expect("an outside app's line");
    assert_eq!(line.size.height, px(FROM_LINE));
    assert_eq!(view.read_with(cx, |view, _| view.reply_drawn.get()), None);
    let row = view.read_with(cx, |view, _| view.page_row_for(30, None));
    let long = "Claude Code ".repeat(17);
    for name in [long.as_str(), "Needs you", "Done · 13:31", "Working", ""] {
        let (session, talk) = named(name);
        let (_, _, view, cx) = standing_with(session, talk, cx);
        let drawn = view.read_with(cx, |view, _| view.from_drawn.get());
        assert_eq!(drawn, Some(line), "{name:?}");
        let theirs = view.read_with(cx, |view, _| view.page_row_for(30, None));
        assert_eq!(theirs, row, "{name:?}");
    }
    // A task's panel has none, even named as an app.
    let task = Card {
        name: "Claude Code".into(),
        ..card()
    };
    let (_, _, view, cx) = standing_with(task, conversation(3), cx);
    assert_eq!(view.read_with(cx, |view, _| view.from_drawn.get()), None);
}

/// An outside app's session, standing on tab 30: its card, with no composer, and its step lines.
fn session_panel() -> (Card, Conversation) {
    let session = Card {
        agent: "session-1".into(),
        name: "Claude Code".into(),
        replies: false,
        ..card()
    };
    let talk = Conversation {
        agent: "session-1".into(),
        name: "Claude Code".into(),
        lines: vec![
            Line::Step(step(0, "Read the page", false)),
            Line::Step(step(1, "Clicking “Tickets”", true)),
        ],
        ..conversation(0)
    };
    (session, talk)
}

/// With the keyboard in the page, in each of the page row's cases and an outside app's panel: ⌘↩
/// and ⌘. are the browser's own commands and answer as they do with the keyboard in the UI, Take
/// Over or Hand Back and Stop, where the case has one; ↩ and Space reach the page and press nothing
/// of ours, even with the ring last on the row's button.
#[gpui::test]
fn from_the_page_the_commands_answer_and_return_and_space_press_nothing(cx: &mut TestAppContext) {
    for case in row_cases() {
        let name = case.name;
        let (shell, host, view, cx) = standing_with(case.card, case.talk, cx);
        if let Some(status) = case.status {
            shell.update(cx, |shell, cx| {
                shell.agent_acted(status.agent.clone(), 30, cx);
                shell.set_agent_status(vec![status], cx)
            });
            redraw(&shell, &view, cx);
        }
        // The ring was last on the row's button, and the page took the keyboard.
        if let Some(button) = case.button {
            shell.update(cx, |shell, cx| shell.key_strip(button, cx));
        }
        shell.update(cx, |shell, cx| shell.leave_strip("page", cx));
        host.page_keys.set(Some(true));
        host.take();
        let from_page = |key: &str, cx: &mut VisualTestContext| {
            let kept = shell.update(cx, |shell, cx| {
                shell.run_agent_page_key(key, cx) || shell.run_ilo_page_key(key, false, cx)
            });
            cx.run_until_parked();
            (kept, host.take())
        };
        for key in ["enter", "space"] {
            let (kept, calls) = from_page(key, cx);
            assert!(!kept, "{name}: {key} from the page was kept from it");
            assert_eq!(
                calls,
                Vec::<String>::new(),
                "{name}: {key} from the page pressed"
            );
        }
        let (kept, calls) = from_page("cmd-enter", cx);
        match case.command {
            Some(command) => assert!(
                kept && calls.iter().any(|call| call == command),
                "{name}: ⌘↩ from the page didn't ask {command}: {calls:?}"
            ),
            None => assert!(!kept && calls.is_empty(), "{name}: ⌘↩ answered: {calls:?}"),
        }
        let (kept, calls) = from_page("cmd-.", cx);
        match case.stops {
            false => assert!(!kept && calls.is_empty(), "{name}: ⌘. answered: {calls:?}"),
            true => assert!(
                kept && calls
                    .iter()
                    .any(|call| call.contains("Stop") || call.starts_with("stop")),
                "{name}: ⌘. from the page didn't stop: {calls:?}"
            ),
        }
    }
}

/// The page row has one button at a time, by what the thread is doing on the page shown, and
/// none for a page the thread isn't on; an outside app's session adds its Stop while it works.
#[gpui::test]
fn the_page_row_holds_one_case_at_a_time(cx: &mut TestAppContext) {
    use super::{PageRow, RowCase};
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let row = |talk: Conversation, open: Option<&str>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
        view.read_with(cx, |view, _| {
            view.page_row_for(30, open.map(str::to_string))
        })
    };
    let working = row(conversation(3), None, cx);
    assert!(
        matches!(working.case, Some(RowCase::Working(_))),
        "{working:?}"
    );
    assert!(!working.stop);
    let held = Conversation {
        live: Live::Yours {
            tab: 30,
            site: None,
        },
        ..conversation(3)
    };
    assert!(matches!(row(held, None, cx).case, Some(RowCase::Yours(_))));
    let elsewhere = Conversation {
        on: Some(31),
        ..conversation(3)
    };
    assert_eq!(
        row(elsewhere, None, cx),
        PageRow::default(),
        "a row for a page it isn't on"
    );
    let ended = Conversation {
        stoppable: false,
        ..conversation(3)
    };
    assert_eq!(
        row(ended.clone(), Some("Open skyscanner.net"), cx).case,
        Some(RowCase::Ended("Open skyscanner.net".into()))
    );
    assert_eq!(row(ended, None, cx), PageRow::default());
}

/// The panel's head has the page shown's speaker only while the page plays or is muted, and its
/// click mutes or unmutes the page as a row's does.
#[gpui::test]
fn the_panels_speaker_is_there_while_the_page_plays_or_is_muted(cx: &mut TestAppContext) {
    let (shell, host, view, cx) = standing_with(card(), conversation(3), cx);
    let speaker = |cx: &mut VisualTestContext| {
        redraw(&shell, &view, cx);
        view.read_with(cx, |view, _| view.speaker_drawn.get())
    };
    assert_eq!(speaker(cx), None, "a silent page has a speaker");
    let playing = Page {
        url: "https://skyscanner.net/".into(),
        audible: true,
        ..Default::default()
    };
    shell.update(cx, |shell, cx| shell.set_page(30, playing.clone(), cx));
    let at = speaker(cx).expect("a playing page has no speaker");
    cx.simulate_click(at.center(), Modifiers::none());
    assert_eq!(*host.muted.borrow(), [(30, true)]);
    let muted = Page {
        audible: false,
        muted: true,
        ..playing
    };
    shell.update(cx, |shell, cx| shell.set_page(30, muted, cx));
    assert!(speaker(cx).is_some(), "a muted page has no speaker");
}

/// After a quit, a kept card's plate in Latest tasks shows the picture its card read back; a card
/// with none has no plate.
#[gpui::test]
fn a_kept_cards_plate_shows_the_picture_it_read_back(cx: &mut TestAppContext) {
    let pixels = image::RgbaImage::from_pixel(3, 2, image::Rgba([40, 80, 120, 255]));
    let image = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)]));
    let plate = crate::agents::kept::cards::Plate {
        image,
        size: (3, 2),
    };
    let kept = Card {
        kept_picture: Some(plate),
        ..card()
    };
    let (shell, _, cx) = shown_with(kept.clone(), cx);
    let size = |card: &Card, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.plate_of(card).map(|p| p.size()))
    };
    assert_eq!(size(&kept, cx), Some((3., 2.)));
    assert_eq!(size(&card(), cx), None, "a plate with no picture");
}

/// A kept card's picture that came back goes to the pass that frees pictures from the window's
/// atlas once no card holds it: when its task is replied to and runs again, or when the card is
/// removed. Every window is given the cards, so every window that drew it lets it go.
#[gpui::test]
fn a_kept_cards_picture_goes_when_its_card_does(cx: &mut TestAppContext) {
    let pixels = image::RgbaImage::from_pixel(3, 2, image::Rgba([40, 80, 120, 255]));
    let image = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)]));
    let plate = crate::agents::kept::cards::Plate {
        image: image.clone(),
        size: (3, 2),
    };
    let kept = Card {
        kept_picture: Some(plate),
        ..card()
    };
    let (shell, _, cx) = shown_with(kept.clone(), cx);
    // What the cards' change hands on, read before the next frame lets it go.
    let freed = |cards: Vec<Card>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.stale_pictures.clear();
            shell.set_task_cards(cards, cx);
            (shell.stale_pictures.iter()).any(|stale| std::sync::Arc::ptr_eq(stale, &image))
        })
    };
    let retitled = Card {
        title: "Still kept".into(),
        ..kept.clone()
    };
    assert!(
        !freed(vec![retitled.clone()], cx),
        "a card that still holds it"
    );
    let replied = Card {
        kept_picture: None,
        ..retitled.clone()
    };
    assert!(freed(vec![replied], cx), "a reply");
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![retitled], cx));
    assert!(freed(Vec::new(), cx), "Remove or Remove All");
}

/// A picture the conversation takes goes to its task's kept card only if it could have been
/// given to an agent both when it was asked and when it came: by the agents' rules, and not while
/// the page uses the camera or the microphone, shares a screen, window or tab, or is captured. A
/// private window tells the card nothing.
#[gpui::test]
fn a_kept_card_is_told_only_of_a_picture_an_agent_could_have_had(cx: &mut TestAppContext) {
    let card = Card {
        last_tab: Some(30),
        pages: vec![30],
        ..card()
    };
    let (shell, host, cx) = shown_with(card, cx);
    host.keeps.borrow_mut().insert(30);
    let steps = std::cell::Cell::new(0);
    // The conversation moves on, which asks for its page's picture; the core answers, with one
    // if `came`, once `before` has changed the page.
    let answered = |before: &dyn Fn(&mut Shell, &mut gpui::Context<Shell>),
                    came: bool,
                    cx: &mut VisualTestContext| {
        steps.set(steps.get() + 1);
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(conversation(steps.get())), cx)
        });
        let request = picture_request(&shell, 30, cx);
        shell.update(cx, |shell, cx| {
            before(shell, cx);
            let picture = PagePicture::from_bgra(vec![255; 4 * 4], 2, 2).filter(|_| came);
            shell.page_pictured(request, picture, cx)
        });
        host.card_pictures.borrow_mut().pop()
    };
    let taken = |before: &dyn Fn(&mut Shell, &mut gpui::Context<Shell>),
                 cx: &mut VisualTestContext| answered(before, true, cx);
    let told = |keeps| Some(("task-1".into(), 30, keeps));
    assert_eq!(taken(&|_, _| {}, cx), told(true));
    type Use = fn(&mut Page);
    let uses: [Use; 4] = [
        |page| page.camera = true,
        |page| page.microphone = true,
        |page| page.sharing = true,
        |page| page.shared = true,
    ];
    for using in uses {
        let mut page = Page::default();
        using(&mut page);
        // In use when it came, and when it was asked.
        let used = page.clone();
        let set = move |shell: &mut Shell, cx: &mut gpui::Context<Shell>| {
            shell.set_page(30, used.clone(), cx)
        };
        assert_eq!(taken(&set, cx), told(false));
        assert_eq!(taken(&|_, _| {}, cx), told(false));
        let unset = |shell: &mut Shell, cx: &mut gpui::Context<Shell>| {
            shell.set_page(30, Page::default(), cx)
        };
        // Asked while in use, it came after: still not.
        assert_eq!(taken(&unset, cx), told(false));
        assert_eq!(taken(&|_, _| {}, cx), told(true));
    }
    // The agents' rules say no, as for a page the user held.
    host.keeps.borrow_mut().clear();
    assert_eq!(taken(&|_, _| {}, cx), told(false));
    // With no picture either: the card's older one goes.
    assert_eq!(answered(&|_, _| {}, false, cx), told(false));
    host.keeps.borrow_mut().insert(30);
    assert_eq!(taken(&|_, _| {}, cx), told(true));
    // No picture of a page that could be kept: the card keeps what it had.
    assert_eq!(answered(&|_, _| {}, false, cx), None);

    shell.update(cx, |shell, _| shell.private = true);
    assert_eq!(taken(&|_, _| {}, cx), None);
}

/// Task-1's conversation of `lines` lines of its own and then `last`, which scroll.
fn foot_talk(lines: usize, last: Vec<Line>) -> Conversation {
    let mut talk = conversation(1);
    (talk.lines).extend((0..lines).map(|n| {
        Line::Said(
            format!("Line {n} of what the assistant said, long enough to wrap in the panel").into(),
        )
    }));
    talk.lines.extend(last);
    talk
}

/// ilo's line handing tab 30 to the user, with Hand Back and Not Now.
fn handed(key: usize) -> Line {
    Line::Ilo(IloLine {
        key,
        kind: IloKind::Yours {
            tab: 30,
            site: None,
        },
        settled: false,
    })
}

/// ilo's line that it stopped the task at an order on tab 30, with Take Over.
fn stopped(key: usize) -> Line {
    Line::Ilo(IloLine {
        key,
        kind: IloKind::Stopped {
            tab: 30,
            at: "Place order".into(),
            doing: "clicking “Place order”".into(),
            why: Why::Order,
            tier: Tier::Work,
            count: 1,
        },
        settled: false,
    })
}

/// The product puts the handed page 30 beside the conversation by itself, as at a hand-off
/// (`open_needing_page`), and the core then shows its Space and page.
fn hand_off(shell: &Entity<Shell>, view: &Entity<super::TalkView>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.open_needing_page(30, cx));
    shell.update(cx, |shell, cx| {
        shell.set_space(3, cx);
        shell.set_selected(30, cx);
    });
    redraw(shell, view, cx);
    redraw(shell, view, cx);
    assert!(
        shell.read_with(cx, |shell, _| shell.task_standing()),
        "the handed page isn't beside the conversation"
    );
}

/// Whether line `key`'s button was drawn whole inside the lines' scrolled area, by the scroll's
/// own bounds and the place the button recorded as it was painted.
fn in_view(
    view: &Entity<super::TalkView>,
    key: (usize, IloButton),
    cx: &mut VisualTestContext,
) -> Result<(), String> {
    let (button, _) = ilo_button(view, key, cx).ok_or(format!("{key:?} wasn't drawn"))?;
    let area = view.read_with(cx, |view, _| view.scroll.bounds());
    match button.top() >= area.top() && button.bottom() <= area.bottom() {
        true => Ok(()),
        false => Err(format!(
            "{key:?} drawn at {:?} to {:?}, the lines' area {:?} to {:?}",
            button.top(),
            button.bottom(),
            area.top(),
            area.bottom()
        )),
    }
}

/// The scroll's offset and its most, as last laid out.
fn foot_scrolled(
    view: &Entity<super::TalkView>,
    cx: &mut VisualTestContext,
) -> (gpui::Pixels, gpui::Pixels) {
    view.read_with(cx, |view, _| {
        (view.scroll.offset().y, view.scroll.max_offset().y)
    })
}

/// At a hand-off the product puts the handed page beside the conversation, whose column narrows
/// and wraps taller: read at its foot before, it's at its foot after, so the handed line's Hand
/// Back and Not Now stand in view, and a line that needs the user after it is followed.
#[gpui::test]
fn a_hand_off_keeps_a_conversation_at_its_foot(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, Vec::new())), cx)
    });
    redraw(&shell, &view, cx);
    let (offset, max) = foot_scrolled(&view, cx);
    assert!(max > px(0.), "the lines fit, so nothing scrolls");
    assert_eq!(offset, -max, "it opened away from its foot");

    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900)])), cx)
    });
    redraw(&shell, &view, cx);
    hand_off(&shell, &view, cx);
    for button in [IloButton::HandBack, IloButton::NotNow] {
        in_view(&view, (900, button), cx).unwrap_or_else(|miss| {
            panic!("beside the handed page, the handed line's button is out of view: {miss}")
        });
    }

    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900), stopped(901)])), cx)
    });
    redraw(&shell, &view, cx);
    in_view(&view, (901, IloButton::TakeOver), cx).unwrap_or_else(|miss| {
        panic!("a line that needs the user, beside the handed page, wasn't followed: {miss}")
    });
}

/// The same with the lines still easing after the line before when the page goes beside: the
/// follow ends at the foot of the narrower column, not short of it.
#[gpui::test]
fn a_follow_under_way_at_a_hand_off_ends_at_the_foot(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, Vec::new())), cx)
    });
    redraw(&shell, &view, cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    // The question comes wide, at the foot: the lines start to follow it on their curve.
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![stopped(900)])), cx)
    });
    redraw(&shell, &view, cx);
    assert!(
        view.read_with(cx, |view, _| view.motion.follow.is_some()),
        "the question wasn't followed on a curve"
    );
    // The curve takes a step wide, then the product hands the user the page in the same moment
    // as its line comes, and the curve goes on a step a frame to its end.
    let step = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.update(cx, |view, _| {
            if let Some(follow) = view.motion.follow.as_mut() {
                follow.since -= std::time::Duration::from_millis(40);
            }
        });
    };
    step(&view, cx);
    redraw(&shell, &view, cx);
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![stopped(900), handed(901)])), cx)
    });
    hand_off(&shell, &view, cx);
    for _ in 0..100 {
        if view.read_with(cx, |view, _| view.motion.follow.is_none()) {
            break;
        }
        step(&view, cx);
        redraw(&shell, &view, cx);
    }
    redraw(&shell, &view, cx);
    let (offset, max) = foot_scrolled(&view, cx);
    assert_eq!(offset, -max, "the follow under way ended short of the foot");
    for button in [IloButton::HandBack, IloButton::NotNow] {
        in_view(&view, (901, button), cx).unwrap_or_else(|miss| {
            panic!("the handed line's button is out of view after the follow: {miss}")
        });
    }
}

/// Scrolled up before the hand-off, the lines stay where the user left them, and nothing that
/// comes after moves them: the card's Needs you tells them.
#[gpui::test]
fn a_hand_off_leaves_lines_the_user_scrolled_up_where_they_are(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, Vec::new())), cx)
    });
    redraw(&shell, &view, cx);
    view.update(cx, |view, _| {
        view.scroll.set_offset(point(px(0.), px(-100.)))
    });
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900)])), cx)
    });
    redraw(&shell, &view, cx);
    hand_off(&shell, &view, cx);
    assert_eq!(
        foot_scrolled(&view, cx).0,
        px(-100.),
        "the hand-off pulled the user away from what they read"
    );
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900), stopped(901)])), cx)
    });
    redraw(&shell, &view, cx);
    assert_eq!(
        foot_scrolled(&view, cx).0,
        px(-100.),
        "a line after the hand-off pulled the user away from what they read"
    );
}

/// Back from the page, the column wide again, a conversation that was at its foot still is.
#[gpui::test]
fn back_from_a_handed_page_a_conversation_stays_at_its_foot(cx: &mut TestAppContext) {
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900)])), cx)
    });
    redraw(&shell, &view, cx);
    hand_off(&shell, &view, cx);
    let (offset, max) = foot_scrolled(&view, cx);
    assert_eq!(offset, -max, "beside the page, it's off its foot");
    shell.update(cx, |shell, cx| shell.task_page_back(cx));
    redraw(&shell, &view, cx);
    redraw(&shell, &view, cx);
    assert!(
        !shell.read_with(cx, |shell, _| shell.task_standing()),
        "the page didn't go back"
    );
    let (offset, max) = foot_scrolled(&view, cx);
    assert_eq!(offset, -max, "back wide, it's off its foot");
    // And a line that comes then is followed.
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(foot_talk(60, vec![handed(900), stopped(901)])), cx)
    });
    redraw(&shell, &view, cx);
    in_view(&view, (901, IloButton::TakeOver), cx)
        .unwrap_or_else(|miss| panic!("back wide, a new line wasn't followed: {miss}"));
}

/// The words a thread's model trigger says for each assistant in Cli's own lists, its default
/// first, then one longer than any of them, as a CLI can list: whole, and the model's own word, a
/// default's being the model menu's `Default`.
fn models_in_cli_lists() -> Vec<(String, String)> {
    use crate::agents::runner::Cli;
    let mut models = Vec::new();
    for cli in Cli::ALL {
        let default = crate::agents::task::model_default(cli);
        models.push((default, crate::command_bar::DEFAULT_ROW.to_string()));
        for (_, name) in cli.unlisted_models() {
            models.push((format!("{} \u{b7} {name}", cli.name()), name.to_string()));
        }
    }
    let long = "Sonnet 4.5 with a context of a million";
    models.push((format!("Claude \u{b7} {long}"), long.into()));
    models
}

/// How wide `words` are at the panel's size, laid out whole, and their first word alone.
fn panel_widths(words: &str, cx: &mut VisualTestContext) -> (f32, f32) {
    use crate::tokens::shell::conversation::panel::COMPOSER_LEVEL;
    let first = words
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    cx.update(|_, cx| {
        let system = WindowTextSystem::new(cx.text_system().clone());
        let width = |text: &str| {
            text_width(
                &text.to_string().into(),
                COMPOSER_LEVEL,
                FontWeight::NORMAL,
                &system,
            )
        };
        (width(words), width(&first))
    })
}

/// The composer's inside edges in the 300 panel: the panel less its foot at each side and the
/// composer's line of 1.
fn composer_inside(panel: gpui::Bounds<gpui::Pixels>) -> (gpui::Pixels, gpui::Pixels) {
    use crate::tokens::shell::conversation::panel::FOOT;
    (panel.left() + px(FOOT + 1.), panel.right() - px(FOOT + 1.))
}

/// The panel's composer row fits the panel beside a page (300) with its plus drawn: the round
/// button's right edge is the row's inset inside the composer, at Stop and at Send, at every
/// level, for every model in Cli's lists. The model trigger draws the model's own word; nothing
/// of the row is cut but that word, which gives way at its end and is never narrower than its
/// first word. The level's words are always whole. The one difference the lead named for now
/// (26 Nov): Work on pages with a default's row stands exactly 1 over, at Stop and at Send.
#[gpui::test]
fn the_panels_composer_row_fits_the_panel_with_its_plus(_: &mut TestAppContext) {
    use crate::tokens::command_bar::{TRIGGER_CHEVRON, TRIGGER_GAP};
    use crate::tokens::shell::conversation::COMPOSER_ROW_GAP;
    use crate::tokens::shell::conversation::panel::{
        COMPOSER_LEVEL_X, COMPOSER_ROUND, COMPOSER_ROW_X,
    };
    let (_, long) = models_in_cli_lists().pop().unwrap();
    let mut cut = false;
    let (mut fit, mut named) = (0, 0);
    for tier in Tier::ALL {
        for (said, word) in models_in_cli_lists() {
            for stoppable in [true, false] {
                let mut each = TestAppContext::single();
                let talk = Conversation {
                    stoppable,
                    tier,
                    model: Some((
                        "icons/terminal.svg",
                        said.clone().into(),
                        word.clone().into(),
                        true,
                    )),
                    ..conversation(3)
                };
                let (_, _, view, cx) = standing_with(card(), talk, &mut each);
                let case = format!("{tier:?}, {said:?}, Stop {stoppable}");
                let (panel, round, plus, level, model) = view.read_with(cx, |view, _| {
                    (
                        view.drawn_at.get().expect("no panel"),
                        view.round_drawn.get().expect("no round button"),
                        view.plus.drawn().expect("no plus"),
                        view.level_drawn.get().expect("no level"),
                        view.model_drawn.get().expect("no model"),
                    )
                });
                assert_eq!(panel.size.width, px(BESIDE), "{case}: the panel's width");
                assert_eq!(
                    round.size.width,
                    px(COMPOSER_ROUND),
                    "{case}: the round button"
                );
                let (left, right) = composer_inside(panel);
                let over = tier == Tier::Work && word == crate::command_bar::DEFAULT_ROW;
                assert_eq!(
                    round.right(),
                    right - px(COMPOSER_ROW_X) + px(if over { 1. } else { 0. }),
                    "{case}: the round button's right edge isn't the row's inset inside the composer{}",
                    if over {
                        ", 1 over as the lead named"
                    } else {
                        ""
                    }
                );
                *if over { &mut named } else { &mut fit } += 1;
                let start = left + px(COMPOSER_ROW_X);
                let end = round.left() - px(COMPOSER_ROW_GAP);
                for (name, item) in [("plus", plus), ("level", level), ("model", model)] {
                    assert!(
                        item.left() >= start && item.right() <= end,
                        "{case}: the {name} is drawn at {:?}..{:?}, outside the row's {start:?}..{end:?}",
                        item.left(),
                        item.right()
                    );
                }
                // The level whole: its insets, its words and its chevron.
                let whole = panel_widths(tier.label(), cx).0
                    + 2. * COMPOSER_LEVEL_X
                    + TRIGGER_GAP
                    + TRIGGER_CHEVRON;
                assert!(
                    f32::from(level.size.width) + 1. >= whole,
                    "{case}: the level is {:?} wide, its words cut (whole it's {whole})",
                    level.size.width
                );
                // The model's word: the trigger less its insets and its chevron, as it opens.
                let drawn = f32::from(model.size.width)
                    - 2. * COMPOSER_LEVEL_X
                    - TRIGGER_GAP
                    - TRIGGER_CHEVRON;
                let (whole, least) = panel_widths(&word, cx);
                assert!(
                    drawn + 1. >= least,
                    "{case}: the model's word is {drawn} wide, under its first word ({least})"
                );
                // Cut only where it has no room: it then stands against the round button.
                if drawn + 1. < whole {
                    assert!(
                        model.right() + px(1.) >= end,
                        "{case}: {word:?} is cut to {drawn} with room left before the button"
                    );
                    cut |= word == long;
                }
            }
        }
    }
    assert!(
        cut,
        "the longest model's word was never cut, so this tests no cut"
    );
    // Each assistant's default at Work on pages, at Stop and at Send: 1 over; every other fits.
    assert_eq!(
        (fit, named),
        (32, 4),
        "(cases that fit, Work on pages with a default 1 over)"
    );
}

/// In the panel the model trigger draws the model's own word alone, without its assistant's
/// name.
#[gpui::test]
fn the_panels_model_trigger_draws_the_models_own_word(cx: &mut TestAppContext) {
    use crate::tokens::shell::conversation::panel::COMPOSER_LEVEL_X;
    let talk = Conversation {
        model: Some((
            "icons/terminal.svg",
            "Claude \u{b7} Sonnet".into(),
            "Sonnet".into(),
            false,
        )),
        ..conversation(3)
    };
    let (_, _, view, cx) = standing_with(card(), talk, cx);
    let model = view
        .read_with(cx, |view, _| view.model_drawn.get())
        .expect("no model");
    // A model that can't change has no chevron: the trigger is its insets and its word.
    let drawn = f32::from(model.size.width) - 2. * COMPOSER_LEVEL_X;
    let (sonnet, _) = panel_widths("Sonnet", cx);
    let (both, _) = panel_widths("Claude \u{b7} Sonnet", cx);
    assert!(
        (drawn - sonnet).abs() <= 1.,
        "the panel's model is {drawn} wide: Sonnet alone is {sonnet}, with Claude's name {both}"
    );
}

/// The panel's row alone, at the composer's width in the 300 panel and at a wider one, with and
/// without the plus, at Stop and at Send: the round button stands the row's inset inside, and the
/// model's word is cut at 300 only, never under its first word, and whole again where there's
/// room.
#[gpui::test]
fn the_panels_row_cuts_the_model_only_where_it_hasnt_room(cx: &mut TestAppContext) {
    use crate::tokens::shell::conversation::panel::{COMPOSER_LEVEL_X, COMPOSER_ROW_X, FOOT};
    use gpui::prelude::*;
    use gpui::{Bounds, IntoElement, Pixels, SharedString, Window};
    use std::cell::Cell;
    type At = Rc<Cell<Option<Bounds<Pixels>>>>;
    struct Drawn {
        width: f32,
        plus: bool,
        round: choices::Round,
        word: SharedString,
        least: f32,
        model: At,
        round_at: At,
    }
    impl gpui::Render for Drawn {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let ink = gpui::black();
            let trigger = |id, at: At| choices::Trigger {
                id,
                at,
                click: None,
            };
            let look = choices::plus_look(ink, ink, 16., 6., false);
            let plus = (self.plus).then(|| {
                crate::plus::Plus::default().draw("plus", Vec::new(), &look, cx.entity_id())
            });
            let round_at = self.round_at.clone();
            gpui::div()
                .font_family(crate::face::UI)
                .w(px(self.width))
                .flex()
                .flex_col()
                .child(choices::row(choices::Row {
                    form: super::Composer::of(true).row,
                    inks: choices::Inks {
                        text: ink,
                        dim: ink,
                        plate: ink,
                        primary: ink,
                        on_primary: ink,
                    },
                    a11y: false,
                    plus,
                    added: Vec::new(),
                    level: (Tier::Work, trigger("level", Rc::default())),
                    model: Some((
                        None,
                        self.word.clone(),
                        self.word.clone(),
                        trigger("model", self.model.clone()),
                    )),
                    model_least: self.least,
                    effort: None,
                    held: Vec::new(),
                    open: None,
                    round: ("round", "Send".into(), self.round, Box::new(|_, _, _| {})),
                    round_at: Some(Box::new(move |bounds| round_at.set(Some(bounds)))),
                }))
        }
    }
    let (_, long) = models_in_cli_lists().pop().unwrap();
    // The composer's row inside the 300 panel: its foot at each side and the composer's line.
    let narrow = BESIDE - 2. * (FOOT + 1.);
    for width in [narrow, narrow + 240.] {
        for plus in [true, false] {
            for round in [choices::Round::Stop, choices::Round::Go { ready: false }] {
                let case = format!("{width}, plus {plus}, {round:?}");
                let least = cx.update(|cx| {
                    use crate::tokens::shell::conversation::panel::COMPOSER_LEVEL;
                    let system = WindowTextSystem::new(cx.text_system().clone());
                    let first = long.split_whitespace().next().unwrap().to_string().into();
                    text_width(&first, COMPOSER_LEVEL, FontWeight::NORMAL, &system).ceil()
                });
                let (model, round_at) = (At::default(), At::default());
                let (word, at, drawn) = (long.clone().into(), model.clone(), round_at.clone());
                let (_, cx) = cx.add_window_view(|_, _| Drawn {
                    width,
                    plus,
                    round,
                    word,
                    least,
                    model: at,
                    round_at: drawn,
                });
                cx.run_until_parked();
                let round = round_at.get().expect("no round button");
                assert_eq!(round.right(), px(width - COMPOSER_ROW_X), "{case}");
                let model = model.get().expect("no model");
                assert!(
                    model.right() <= round.left(),
                    "{case}: the model past the button"
                );
                let shown = f32::from(model.size.width) - 2. * COMPOSER_LEVEL_X;
                let (whole, first) = panel_widths(&long, cx);
                assert!(shown + 1. >= first, "{case}: under its first word");
                match width == narrow {
                    true => assert!(shown + 1. < whole, "{case}: not cut where it hasn't room"),
                    false => assert!(shown + 1. >= whole, "{case}: cut where there's room"),
                }
            }
        }
    }
}

/// The sign over the composer as last drawn: what it said, and where.
fn sign_drawn(
    view: &Entity<super::TalkView>,
    cx: &mut VisualTestContext,
) -> Option<(super::Knot, gpui::Bounds<gpui::Pixels>)> {
    let drawn = view.read_with(cx, |view, _| view.sign_drawn.get());
    drawn.map(|(sign, _, bounds)| (sign, bounds))
}

/// The sign says working while the thread's step or turn is under way, waiting while it waits on
/// the user in each way it can, and is gone once the thread can't be stopped, read from the
/// thread's own state alone: an assistant's words that say it waits change nothing.
#[gpui::test]
fn the_sign_says_working_waiting_or_nothing_from_the_threads_own_state(cx: &mut TestAppContext) {
    use super::Knot::{Waiting, Working};
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let yours = Live::Yours {
        tab: 30,
        site: None,
    };
    let mut talk = conversation(1);
    talk.lines
        .push(Line::Said("I'm waiting for you to sign in.".into()));
    for (live, stoppable, sign) in [
        (Live::Working, true, Some(Working)),
        (Live::Refused, true, Some(Waiting)),
        (Live::Handed, true, Some(Waiting)),
        (yours, true, Some(Waiting)),
        (Live::Asked, true, Some(Waiting)),
        (Live::Working, false, None),
        (Live::Asked, false, None),
        (Live::Working, true, Some(Working)),
    ] {
        talk.live = live.clone();
        talk.stoppable = stoppable;
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(talk.clone()), cx)
        });
        redraw(&shell, &view, cx);
        assert_eq!(
            sign_drawn(&view, cx).map(|(sign, _)| sign),
            sign,
            "{live:?}, stoppable {stoppable}"
        );
    }
}

/// The sign stands over the composer's right corner, its right edge `right` in from the
/// composer's and its foot `above` over it, `size` across, in the stage and in the panel beside a
/// page alike.
#[gpui::test]
fn the_sign_stands_over_the_composers_right_corner_in_the_stage_and_beside_a_page(
    cx: &mut TestAppContext,
) {
    use crate::tokens::shell::conversation::sign::{ABOVE, RIGHT, SIZE};
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let mut talk = foot_talk(2, vec![handed(1)]);
    talk.live = Live::Yours {
        tab: 30,
        site: None,
    };
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(talk.clone()), cx)
    });
    redraw(&shell, &view, cx);
    let placed = |cx: &mut VisualTestContext, at: &str| {
        let (_, sign) = sign_drawn(&view, cx).unwrap_or_else(|| panic!("no sign {at}"));
        let composer = view
            .read_with(cx, |view, _| view.composer_drawn.get())
            .unwrap_or_else(|| panic!("no composer {at}"));
        let (right, top) = (composer.right(), composer.top());
        assert_eq!(sign.size.width, px(SIZE), "{at}");
        assert_eq!(sign.size.height, px(SIZE), "{at}");
        assert_eq!(right - sign.right(), px(RIGHT), "{at}");
        assert_eq!(top - sign.bottom(), px(ABOVE), "{at}");
    };
    placed(cx, "in the stage");
    hand_off(&shell, &view, cx);
    placed(cx, "beside a page");
}

/// The gap in the sign's cord as it was drawn last, if it was drawn moving.
fn sign_gap(view: &Entity<super::TalkView>, cx: &mut VisualTestContext) -> Option<(f32, f32)> {
    view.read_with(cx, |view, _| view.sign_drawn.get())
        .and_then(|(_, gap, _)| gap)
}

/// A thread at work, its turn under way with no step: nothing shimmers, and the sign alone moves.
fn working_alone() -> Conversation {
    conversation(0)
}

/// While the thread works, a gap of its share of the cord travels round it, a lap in its time at
/// an even pace, from where the cord starts as the work does.
#[test]
fn the_working_gap_travels_round_the_cord_at_an_even_pace() {
    use crate::shell::knot::{Knot, gap};
    use crate::tokens::shell::conversation::sign::{LAP, WORKING_GAP};
    for (gone, at) in [
        (0., 0.),
        (LAP / 4., 0.25),
        (LAP / 2., 0.5),
        (LAP, 0.),
        (LAP * 1.75, 0.75),
    ] {
        let (was, length) = gap(Knot::Working, gone);
        assert!((was - at).abs() < 1e-4, "{gone} s in: at {was}, not {at}");
        assert_eq!(length, WORKING_GAP, "{gone} s in");
    }
}

/// The working sign moves on the shimmer's clock, which it keeps going with nothing else moving,
/// and asks for no frame with motion reduced, where it stands whole and still, or while its window
/// isn't shown.
#[gpui::test]
fn the_working_sign_asks_for_frames_only_while_it_can_be_seen_moving(cx: &mut TestAppContext) {
    use crate::tokens::shell::conversation::sign::WORKING_GAP;
    let (shell, _, cx) = shown(cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    shell.update(cx, |shell, cx| {
        shell.set_conversation(Some(working_alone()), cx)
    });
    redraw(&shell, &view, cx);
    let pending =
        |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.breath.pending());
    assert!(
        !view.read_with(cx, |view, _| view.moving(false)),
        "something shimmers"
    );
    assert_eq!(
        sign_drawn(&view, cx).map(|(sign, _)| sign),
        Some(super::Knot::Working)
    );
    assert_eq!(
        sign_gap(&view, cx),
        None,
        "with motion reduced the sign moves"
    );
    assert!(!pending(cx), "with motion reduced the sign asks for frames");

    cx.update(|_, cx| cx.set_reduce_motion(false));
    redraw(&shell, &view, cx);
    let (_, length) = sign_gap(&view, cx).expect("the working sign doesn't move");
    assert_eq!(length, WORKING_GAP);
    assert!(pending(cx), "the working sign isn't on the shimmer's clock");

    cx.simulate_visibility_change(gpui::WindowVisibility::Hidden);
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        redraw(&shell, &view, cx);
    }
    // The sidebar's card of a thread at work keeps the shimmer's clock by itself, so what the sign
    // asks for is read from the sign: drawn still, it asks for no frame.
    assert_eq!(
        sign_gap(&view, cx),
        None,
        "the sign moves in a window not shown"
    );
}

/// While the thread waits on the user, at the top of the upper right loop a gap opens to its share
/// of the cord over `open`, holds, closes over `open`, and the knot rests whole for `rest`, over
/// and over, the gap's middle always at the loop's top.
#[test]
fn the_waiting_gap_opens_and_closes_at_the_loops_top_then_rests() {
    use crate::shell::knot::{Knot, gap};
    use crate::tokens::shell::conversation::sign::{HOLD, OPEN, REST, WAITING_GAP};
    let crown = crate::product::crown();
    let cycle = OPEN + HOLD + OPEN + REST;
    let length = |gone: f32| {
        let (at, length) = gap(Knot::Waiting, gone);
        assert!(
            (at + length / 2. - crown).abs() < 1e-4,
            "{gone} s in the gap's middle is at {}, not the loop's top {crown}",
            at + length / 2.
        );
        length
    };
    assert!(
        length(0.) < 1e-4,
        "the wait doesn't start with the knot whole"
    );
    let opening: Vec<f32> = (0..=4).map(|n| length(OPEN * n as f32 / 4.)).collect();
    assert!(
        opening.windows(2).all(|pair| pair[0] < pair[1]),
        "the gap doesn't open steadily: {opening:?}"
    );
    for gone in [OPEN, OPEN + HOLD / 2., OPEN + HOLD] {
        assert!((length(gone) - WAITING_GAP).abs() < 1e-4, "{gone} s in");
    }
    let closing: Vec<f32> = (0..=4)
        .map(|n| length(OPEN + HOLD + OPEN * n as f32 / 4.))
        .collect();
    assert!(
        closing.windows(2).all(|pair| pair[0] > pair[1]),
        "the gap doesn't close steadily: {closing:?}"
    );
    for gone in [
        OPEN + HOLD + OPEN + 0.01,
        OPEN + HOLD + OPEN + REST / 2.,
        cycle - 0.01,
    ] {
        assert!(length(gone) < 1e-3, "the knot isn't whole {gone} s in");
    }
    assert_eq!(length(cycle + OPEN), length(OPEN), "it doesn't go again");
}

/// At a wait with nothing else under way no line of the log moves and the sign is in its waiting
/// motion, on the shimmer's clock; with a second tab's step under way, that step moves too; after
/// the hand back the next step moves again and the sign works; with the window not shown nothing
/// is scheduled.
#[gpui::test]
fn at_a_wait_the_sign_alone_moves(cx: &mut TestAppContext) {
    use super::Knot;
    let (shell, _, cx) = shown(cx);
    cx.update(|_, cx| cx.set_reduce_motion(false));
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let pending =
        |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.breath.pending());
    let show = |talk: &Conversation, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(talk.clone()), cx)
        });
        // Its trace opens as it first draws, and shimmers from the next frame.
        redraw(&shell, &view, cx);
        redraw(&shell, &view, cx);
    };
    // Its step handed to the user: nothing under way.
    let mut talk = conversation(1);
    if let Line::Trace(trace) = &mut talk.lines[0] {
        trace.items = vec![TraceItem::Step(step(0, "Opened skyscanner.net", false))];
    }
    talk.lines.push(handed(1));
    talk.live = Live::Handed;
    show(&talk, cx);
    assert!(
        !view.read_with(cx, |view, _| view.moving(false)),
        "a line moves at the wait"
    );
    assert_eq!(
        sign_drawn(&view, cx).map(|(sign, _)| sign),
        Some(Knot::Waiting)
    );
    assert!(
        sign_gap(&view, cx).is_some(),
        "the waiting sign doesn't move"
    );
    assert!(pending(cx), "the waiting sign isn't on the shimmer's clock");

    // A step under way on a second tab moves too, while the thread still waits.
    let mut second = talk.clone();
    if let Line::Trace(trace) = &mut second.lines[0] {
        trace
            .items
            .push(TraceItem::Step(step(1, "Opened tap.pt", true)));
    }
    show(&second, cx);
    assert!(
        view.read_with(cx, |view, _| view.moving(false)),
        "the second tab's step holds still"
    );
    assert_eq!(
        sign_drawn(&view, cx).map(|(sign, _)| sign),
        Some(Knot::Waiting)
    );

    // Handed back, the next step moves and the sign works.
    let mut back = conversation(2);
    back.lines.insert(1, handed(1));
    show(&back, cx);
    assert!(
        view.read_with(cx, |view, _| view.moving(false)),
        "the next step holds still"
    );
    assert_eq!(
        sign_drawn(&view, cx).map(|(sign, _)| sign),
        Some(Knot::Working)
    );

    // Waiting again, in a window not shown: nothing is scheduled.
    show(&talk, cx);
    cx.simulate_visibility_change(gpui::WindowVisibility::Hidden);
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        redraw(&shell, &view, cx);
    }
    assert_eq!(
        sign_gap(&view, cx),
        None,
        "the waiting sign moves in a window not shown"
    );
}

/// The sign asks for frames only while it moves where it can be seen: a thread waiting on the user
/// with nothing else under way, its card still, has the shimmer's clock kept for the sign alone;
/// with motion reduced, in a window the system doesn't show, or once the thread has ended, the
/// clock isn't kept for it. It never asks for the display's frames, which only what opens, folds
/// or lands does.
#[gpui::test]
fn the_sign_asks_for_frames_only_while_it_moves_where_it_can_be_seen(cx: &mut TestAppContext) {
    let waits = Card {
        dot: Dot::Waiting,
        wait: Default::default(),
        word: Word::for_dot(Dot::Waiting),
        ..card()
    };
    let (shell, _, cx) = shown_with(waits, cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let mut talk = conversation(1);
    if let Line::Trace(trace) = &mut talk.lines[0] {
        trace.items = vec![TraceItem::Step(step(0, "Opened skyscanner.net", false))];
    }
    talk.lines.push(handed(1));
    talk.live = Live::Handed;
    let show = |talk: &Conversation, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            shell.set_conversation(Some(talk.clone()), cx)
        });
    };
    // Whether the clock's next frame is out once its last has drawn.
    let asked = |cx: &mut VisualTestContext| {
        for _ in 0..3 {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(100));
            redraw(&shell, &view, cx);
        }
        shell.read_with(cx, |shell, _| shell.breath.pending())
    };
    show(&talk, cx);
    assert!(
        !view.read_with(cx, |view, _| view.moving(false)),
        "a line moves at the wait"
    );
    assert!(!asked(cx), "with motion reduced the clock is kept");
    assert_eq!(
        sign_gap(&view, cx),
        None,
        "with motion reduced the sign moves"
    );

    cx.update(|_, cx| cx.set_reduce_motion(false));
    assert!(asked(cx), "the waiting sign doesn't keep the clock");
    assert!(
        sign_gap(&view, cx).is_some(),
        "the waiting sign doesn't move"
    );

    cx.simulate_visibility_change(gpui::WindowVisibility::Hidden);
    assert!(!asked(cx), "in a hidden window the clock is kept");
    cx.simulate_visibility_change(gpui::WindowVisibility::Visible);
    assert!(
        asked(cx),
        "shown again, the waiting sign doesn't keep the clock"
    );

    talk.stoppable = false;
    show(&talk, cx);
    assert!(!asked(cx), "once the thread has ended the clock is kept");
    assert_eq!(
        sign_drawn(&view, cx),
        None,
        "an ended thread draws the sign"
    );
}

/// An ended thread's chip, Limit's Retry or its assistant's fix, stands under its conversation's
/// last line, where the sidebar's line no longer has it, and its click does the act. A thread
/// that isn't ended has none there.
#[gpui::test]
fn an_ended_threads_chip_is_under_its_last_line(cx: &mut TestAppContext) {
    let limit = Card {
        dot: Dot::Ended,
        word: Word::AtLimit,
        retry: true,
        ..card()
    };
    let (shell, host, cx) = shown_with(limit.clone(), cx);
    let view = shell.read_with(cx, |shell, _| shell.talk.as_ref().unwrap().view.clone());
    let talk = Conversation {
        stoppable: false,
        live: Default::default(),
        ..conversation(1)
    };
    shell.update(cx, |shell, cx| shell.set_conversation(Some(talk), cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let chip = |view: &Entity<super::TalkView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| view.chip_drawn.get())
    };
    let retry = chip(&view, cx).expect("Retry wasn't drawn under the conversation's last line");
    host.take();
    cx.simulate_click(retry.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["task task-1 Retry"]);

    let done = Card {
        dot: Dot::Done,
        word: Word::Done,
        retry: false,
        ..limit
    };
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![done], cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(chip(&view, cx), None);
}
