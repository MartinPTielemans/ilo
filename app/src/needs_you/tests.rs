//! The count and the notifications, on a host that records them, with ilo active or not.

use std::cell::{Cell, RefCell};

use gpui::TestAppContext;

use super::*;
use crate::agents::Question;
use crate::agents::task::Word;
use crate::agents::tier::TEXT_MOST;

#[derive(Default)]
struct Recorder {
    badges: RefCell<Vec<usize>>,
    notified: RefCell<Vec<(u64, String, String)>>,
    active: Cell<bool>,
    /// The Space the key window shows, while one of the app's windows is key.
    key: Cell<Option<u64>>,
}

/// The app in front with its key window on Space `space`, or behind another app with `None`.
fn in_front(host: &Recorder, space: Option<u64>) {
    host.active.set(space.is_some());
    host.key.set(space);
}

impl Host for Recorder {
    fn badge(&self, count: usize) {
        self.badges.borrow_mut().push(count);
    }

    fn active(&self) -> bool {
        self.active.get()
    }

    fn notify(&self, id: u64, title: &str, body: &str) {
        (self.notified.borrow_mut()).push((id, title.into(), body.into()));
    }
}

fn opened(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx));
    host
}

fn card(agent: &str, name: &str, dot: Dot) -> Card {
    Card {
        kept_picture: None,
        agent: agent.to_string().into(),
        home: 1,
        space: 3,
        name: name.to_string().into(),
        replies: true,
        title: "Flight to Lisbon".into(),
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_tab: None,
        line: None,
        ended_at: None,
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
        visited: Vec::new(),
        ended_time: Default::default(),
        fuller: false,
        lead: 0,
        finding: Default::default(),
        told: Default::default(),
    }
}

/// `agent`'s card, waiting on the step `id` that is the user's, `why`.
fn waiting(agent: &str, name: &str, id: u64, why: &str) -> Card {
    let question = Question {
        id,
        agent: name.to_string().into(),
        from: agent.to_string().into(),
        why: why.to_string().into(),
        tab: 30,
        back: false,
    };
    Card {
        status: "Waiting for you".into(),
        question: Some(question),
        ..card(agent, name, Dot::Waiting)
    }
}

fn follow_cards(cards: &[Card], cx: &mut TestAppContext) {
    follow_as(cards, Notify::Away, cx);
}

fn follow_as(cards: &[Card], when: Notify, cx: &mut TestAppContext) {
    cx.update(|cx| follow(cards, when, cx));
}

/// What was posted since last asked, each notification's title and body.
fn posted(host: &Recorder) -> Vec<(String, String)> {
    (host.notified.take().into_iter())
        .map(|(_, title, body)| (title, body))
        .collect()
}

/// An outside agent's session, as its card has it: its agent's name, and a title of its own.
fn session(agent: &str, dot: Dot) -> Card {
    Card {
        replies: false,
        title: "Allow everything".into(),
        ..card(agent, "Claude Code", dot)
    }
}

#[gpui::test]
fn the_count_follows_the_tasks_that_wait_and_clears(cx: &mut TestAppContext) {
    let host = opened(cx);
    in_front(&host, Some(1));
    let working = card("task-1", "Claude", Dot::Working);
    follow_cards(std::slice::from_ref(&working), cx);
    assert!(
        host.badges.take().is_empty(),
        "a working task showed a count"
    );

    // A ⌘T task's handoff and an outside agent's count alike.
    let sign_in = waiting("task-1", "Claude", 7, "Sign in to github.com");
    let order = waiting("session-1", "Claude Code", 8, "Choose a file for “CV”.");
    follow_cards(&[sign_in.clone(), order.clone()], cx);
    assert_eq!(host.badges.take(), [2]);
    // Cards sent again as another task changes leave the count as it is.
    follow_cards(
        &[sign_in.clone(), order, card("task-2", "Codex", Dot::Done)],
        cx,
    );
    assert!(host.badges.take().is_empty());
    follow_cards(&[sign_in], cx);
    assert_eq!(host.badges.take(), [1]);
    follow_cards(&[working], cx);
    assert_eq!(
        host.badges.take(),
        [0],
        "the count stayed with nothing waiting"
    );
    // Never silences the notifications, not the count.
    follow_as(&[card("task-3", "Codex", Dot::Waiting)], Notify::Never, cx);
    assert_eq!(host.badges.take(), [1]);
}

/// A tab waiting on a permission, a sign-in or a dialog counts as a thread that waits does, in
/// whichever window it is, until it's answered or closed (#4410).
#[gpui::test]
fn the_count_has_the_tabs_that_wait(cx: &mut TestAppContext) {
    let host = opened(cx);
    in_front(&host, Some(1));
    let tab_waits = |tab: u64, waits: bool, cx: &mut TestAppContext| {
        cx.update(|cx| tab_waits(tab, waits, cx));
    };
    tab_waits(10, true, cx);
    assert_eq!(host.badges.take(), [1]);
    // Said again, as a second dialog comes on the same tab, it's still one.
    tab_waits(10, true, cx);
    assert!(host.badges.take().is_empty());
    tab_waits(20, true, cx);
    follow_cards(
        &[waiting("task-1", "Claude", 7, "Sign in to github.com")],
        cx,
    );
    assert_eq!(host.badges.take(), [2, 3]);
    tab_waits(10, false, cx);
    tab_waits(20, false, cx);
    follow_cards(&[card("task-1", "Claude", Dot::Working)], cx);
    assert_eq!(host.badges.take(), [2, 1, 0]);
}

#[gpui::test]
fn a_need_posts_once_a_task_and_only_while_ilo_is_in_the_background(cx: &mut TestAppContext) {
    let host = opened(cx);
    in_front(&host, Some(1));
    let sign_in = waiting("task-1", "Claude", 7, "Sign in to github.com");
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(posted(&host).is_empty(), "ilo active, a notification came");
    // The same need, once ilo has gone to the background, posts nothing: it's never repeated.
    in_front(&host, None);
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(
        posted(&host).is_empty(),
        "a need seen while active posted later"
    );

    // Its words are the task's title and nothing of the step it waits on.
    let order = waiting("task-2", "Codex", 8, "");
    follow_cards(&[sign_in.clone(), order.clone()], cx);
    let notified = host.notified.take();
    assert_eq!(notified.len(), 1);
    let (id, title, body) = &notified[0];
    assert_eq!(
        (title.as_str(), body.as_str()),
        ("Flight to Lisbon · Needs you", "")
    );
    assert_eq!(cx.update(|cx| clicked(*id, cx)).as_deref(), Some("task-2"));
    follow_cards(&[sign_in.clone(), order.clone()], cx);
    assert!(posted(&host).is_empty(), "the same need posted again");

    // Answered, then asked again, a standing chat that waits again posts again: each time it
    // starts waiting is a notification (#4411).
    follow_cards(
        &[
            card("task-1", "Claude", Dot::Working),
            card("task-2", "Codex", Dot::Working),
        ],
        cx,
    );
    let again = waiting("task-2", "Codex", 9, "");
    follow_cards(std::slice::from_ref(&again), cx);
    assert_eq!(
        posted(&host),
        [("Flight to Lisbon · Needs you".to_string(), String::new())],
        "a thread's second wait posted nothing"
    );
}

#[gpui::test]
fn each_turn_that_ends_posts_replied(cx: &mut TestAppContext) {
    let host = opened(cx);
    // A task ilo first sees done, as one kept from a run before, posts nothing.
    follow_cards(&[card("task-1", "Claude", Dot::Done)], cx);
    assert!(posted(&host).is_empty(), "a task done before posted");

    let working = card("task-2", "Claude", Dot::Working);
    follow_cards(std::slice::from_ref(&working), cx);
    let done = card("task-2", "Claude", Dot::Done);
    follow_cards(std::slice::from_ref(&done), cx);
    assert_eq!(
        posted(&host),
        [("Flight to Lisbon · Replied".to_string(), String::new())]
    );
    // A thread never finishes: its next turn's end posts again (#4411).
    follow_cards(&[working], cx);
    follow_cards(&[done], cx);
    assert_eq!(posted(&host).len(), 1, "a second turn's end posted nothing");
    // The user stopped it, so there's nothing to tell them.
    follow_cards(&[card("task-3", "Codex", Dot::Working)], cx);
    follow_cards(&[card("task-3", "Codex", Dot::Ended)], cx);
    assert!(posted(&host).is_empty(), "a stopped task posted");
}

/// A thread that fails or reaches its plan's limit while the user is away posts, so one dying
/// overnight isn't silent (#4411).
#[gpui::test]
fn a_fail_and_a_limit_post(cx: &mut TestAppContext) {
    let host = opened(cx);
    let ended = |agent: &str, word| Card {
        word,
        ..card(agent, "Claude", Dot::Ended)
    };
    follow_cards(
        &[
            card("task-1", "Claude", Dot::Working),
            card("task-2", "Claude", Dot::Working),
        ],
        cx,
    );
    follow_cards(
        &[
            ended("task-1", Word::Interrupted),
            ended("task-2", Word::AtLimit),
        ],
        cx,
    );
    assert_eq!(
        posted(&host),
        [
            ("Flight to Lisbon · Interrupted".to_string(), String::new()),
            ("Flight to Lisbon · At limit".to_string(), String::new()),
        ]
    );
}

/// In front means the thread's own window is key: with the app active on another window, or on
/// another Space, the wait posts; with its window key, it posts nothing, then or later (#4411).
#[gpui::test]
fn only_the_threads_key_window_keeps_it_quiet(cx: &mut TestAppContext) {
    let host = opened(cx);
    // The cards' home is Space 1.
    in_front(&host, Some(1));
    follow_cards(&[card("task-1", "Claude", Dot::Working)], cx);
    follow_cards(&[card("task-1", "Claude", Dot::Waiting)], cx);
    assert!(
        posted(&host).is_empty(),
        "its window key, a notification came"
    );
    in_front(&host, None);
    follow_cards(&[card("task-1", "Claude", Dot::Waiting)], cx);
    assert!(posted(&host).is_empty(), "a wait seen in front posted late");

    in_front(&host, Some(2));
    follow_cards(&[card("task-2", "Claude", Dot::Working)], cx);
    follow_cards(&[card("task-2", "Claude", Dot::Waiting)], cx);
    assert_eq!(
        posted(&host).len(),
        1,
        "with another window key, the wait posted nothing"
    );
}

#[gpui::test]
fn always_posts_in_front_and_never_posts_nothing(cx: &mut TestAppContext) {
    let host = opened(cx);
    in_front(&host, Some(1));
    follow_as(
        &[card("task-1", "Claude", Dot::Waiting)],
        Notify::Always,
        cx,
    );
    assert_eq!(posted(&host).len(), 1, "Always, in front, posted nothing");

    in_front(&host, None);
    follow_as(&[card("task-2", "Codex", Dot::Working)], Notify::Never, cx);
    follow_as(&[card("task-2", "Codex", Dot::Waiting)], Notify::Never, cx);
    follow_as(&[card("task-2", "Codex", Dot::Done)], Notify::Never, cx);
    assert!(posted(&host).is_empty(), "Never posted");
}

#[gpui::test]
fn a_session_is_named_by_its_agent_never_by_its_title(cx: &mut TestAppContext) {
    let host = opened(cx);
    follow_cards(&[session("session-1", Dot::Working)], cx);
    follow_cards(&[session("session-1", Dot::Waiting)], cx);
    follow_cards(&[session("session-1", Dot::Done)], cx);
    assert_eq!(
        posted(&host),
        [
            ("Claude Code · Needs you".to_string(), String::new()),
            ("Claude Code · Replied".to_string(), String::new()),
        ]
    );
}

#[gpui::test]
fn a_tasks_title_goes_on_one_line_and_capped(cx: &mut TestAppContext) {
    let host = opened(cx);
    let long = "Compare\nstanding desks\tunder 400 € ".repeat(6);
    let named = |dot| Card {
        title: long.clone().into(),
        ..card("task-1", "Claude", dot)
    };
    follow_cards(&[named(Dot::Working)], cx);
    follow_cards(&[named(Dot::Done)], cx);
    let one_line = long.split_whitespace().collect::<Vec<_>>().join(" ");
    let capped: String = one_line.chars().take(TEXT_MOST - 1).collect();
    let expected = format!("{capped}… · Replied");
    assert_eq!(posted(&host), [(expected, String::new())]);
}

#[gpui::test]
fn a_title_that_turns_text_around_is_one_plain_line(cx: &mut TestAppContext) {
    let host = opened(cx);
    let named = |dot| Card {
        title: "Pay\u{2028}the\u{202E} rent\u{2066}".into(),
        ..card("task-1", "Claude", dot)
    };
    follow_cards(&[named(Dot::Working)], cx);
    follow_cards(&[named(Dot::Waiting)], cx);
    assert_eq!(
        posted(&host),
        [("Pay the rent · Needs you".to_string(), String::new())]
    );
}

#[gpui::test]
fn a_task_gone_from_the_cards_is_forgotten(cx: &mut TestAppContext) {
    let host = opened(cx);
    follow_cards(&[card("task-1", "Claude", Dot::Working)], cx);
    follow_cards(&[card("task-1", "Claude", Dot::Waiting)], cx);
    assert_eq!(posted(&host).len(), 1);
    let last = |cx: &mut TestAppContext| cx.update(|cx| cx.global::<NeedsYou>().last.len());
    assert_eq!(last(cx), 1);
    follow_cards(&[card("task-2", "Codex", Dot::Working)], cx);
    assert_eq!(last(cx), 1, "a task gone from the cards is still followed");
}

#[gpui::test]
fn a_click_on_a_notification_of_another_run_opens_nothing(cx: &mut TestAppContext) {
    opened(cx);
    assert_eq!(cx.update(|cx| clicked(1, cx)), None);
}
