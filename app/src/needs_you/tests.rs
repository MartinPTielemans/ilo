//! The count and the notifications, on a host that records them, with ilo active or not.

use std::cell::{Cell, RefCell};

use gpui::TestAppContext;

use super::*;
use crate::agents::Question;
use crate::agents::page::clip;
use crate::agents::task::Word;

#[derive(Default)]
struct Recorder {
    badges: RefCell<Vec<usize>>,
    notified: RefCell<Vec<(u64, String, String)>>,
    active: Cell<bool>,
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
        agent: agent.to_string().into(),
        home: 1,
        space: 3,
        name: name.to_string().into(),
        replies: true,
        title: "Flight to Lisbon".into(),
        status: "Searching flights".into(),
        steps: Vec::new(),
        last_site: None,
        asks: false,
        dot,
        word: Word::for_dot(dot),
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        page_steps: Vec::new(),
        retry: false,
        fix: None,
    }
}

/// `agent`'s card, waiting on question `id`: a handoff, or with no tab, a confirmation.
fn waiting(agent: &str, name: &str, id: u64, handoff: Option<u64>) -> Card {
    let question = Question {
        id,
        agent: name.to_string().into(),
        from: agent.to_string().into(),
        what: "click “Place order” on shop.example".into(),
        why: "Sign in to github.com".into(),
        handoff,
        back: false,
        ..Default::default()
    };
    let status = match handoff {
        Some(_) => "Waiting for you",
        None => "Waiting for your OK",
    };
    Card {
        status: status.into(),
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
    host.active.set(true);
    let working = card("task-1", "Claude", Dot::Working);
    follow_cards(std::slice::from_ref(&working), cx);
    assert!(
        host.badges.take().is_empty(),
        "a working task showed a count"
    );

    // A ⌘T task's handoff and an outside agent's confirmation count alike.
    let sign_in = waiting("task-1", "Claude", 7, Some(30));
    let order = waiting("session-1", "Claude Code", 8, None);
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

#[gpui::test]
fn a_need_posts_once_a_task_and_only_while_ilo_is_in_the_background(cx: &mut TestAppContext) {
    let host = opened(cx);
    host.active.set(true);
    let sign_in = waiting("task-1", "Claude", 7, Some(30));
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(posted(&host).is_empty(), "ilo active, a notification came");
    // The same need, once ilo has gone to the background, posts nothing: it's never repeated.
    host.active.set(false);
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(
        posted(&host).is_empty(),
        "a need seen while active posted later"
    );

    // Its words are the task's title and nothing of the step it waits on.
    let order = waiting("task-2", "Codex", 8, None);
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

    // Answered, then asked again, a task that has posted its need doesn't post another, so going
    // back and forth can't ring the user; one that hadn't, does.
    follow_cards(&[], cx);
    let again = waiting("task-2", "Codex", 9, None);
    follow_cards(std::slice::from_ref(&again), cx);
    assert!(posted(&host).is_empty(), "a task posted its need twice");
    follow_cards(&[again, sign_in], cx);
    assert_eq!(
        posted(&host),
        [("Flight to Lisbon · Needs you".to_string(), String::new())]
    );
}

#[gpui::test]
fn a_task_that_finishes_posts_done_once(cx: &mut TestAppContext) {
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
        [("Flight to Lisbon · Done".to_string(), String::new())]
    );
    follow_cards(&[working], cx);
    follow_cards(&[done], cx);
    assert!(posted(&host).is_empty(), "a task posted its finish twice");
    // A task stopped or failed hasn't finished its work: it posts nothing.
    follow_cards(&[card("task-3", "Codex", Dot::Working)], cx);
    follow_cards(&[card("task-3", "Codex", Dot::Ended)], cx);
    assert!(posted(&host).is_empty(), "a stopped task posted");
}

#[gpui::test]
fn always_posts_in_front_and_never_posts_nothing(cx: &mut TestAppContext) {
    let host = opened(cx);
    host.active.set(true);
    follow_as(
        &[card("task-1", "Claude", Dot::Waiting)],
        Notify::Always,
        cx,
    );
    assert_eq!(posted(&host).len(), 1, "Always, in front, posted nothing");

    host.active.set(false);
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
            ("Claude Code · Done".to_string(), String::new()),
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
    let expected = format!("{} · Done", clip(&one_line, TITLE_CHARS));
    assert_eq!(posted(&host), [(expected, String::new())]);
}

#[gpui::test]
fn a_click_on_a_notification_of_another_run_opens_nothing(cx: &mut TestAppContext) {
    opened(cx);
    assert_eq!(cx.update(|cx| clicked(1, cx)), None);
}
