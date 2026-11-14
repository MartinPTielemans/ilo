//! The count and the notifications, on a host that records them, with ilo active or not.

use std::cell::{Cell, RefCell};

use gpui::TestAppContext;

use super::*;
use crate::agents::Question;

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
        asks: false,
        dot,
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        retry: false,
        command: None,
        point: None,
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
    cx.update(|cx| follow(cards, cx));
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
}

#[gpui::test]
fn each_need_posts_once_and_only_while_ilo_is_in_the_background(cx: &mut TestAppContext) {
    let host = opened(cx);
    host.active.set(true);
    let sign_in = waiting("task-1", "Claude", 7, Some(30));
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(
        host.notified.take().is_empty(),
        "ilo active, a notification came"
    );
    // The same need, once ilo has gone to the background, posts nothing: it's never repeated.
    host.active.set(false);
    follow_cards(std::slice::from_ref(&sign_in), cx);
    assert!(
        host.notified.take().is_empty(),
        "a need seen while active posted later"
    );

    let order = waiting("session-1", "Claude Code", 8, None);
    follow_cards(&[sign_in.clone(), order.clone()], cx);
    let posted = host.notified.take();
    assert_eq!(posted.len(), 1);
    let (id, title, body) = &posted[0];
    assert_eq!(title, "Claude Code needs you");
    assert_eq!(body, "Click “Place order” on shop.example");
    assert_eq!(
        cx.update(|cx| clicked(*id, cx)).as_deref(),
        Some("session-1")
    );
    follow_cards(&[sign_in.clone(), order], cx);
    assert!(
        host.notified.take().is_empty(),
        "the same need posted again"
    );

    // Answered, then asked again: a new question is a new need, whose body is the handoff's step.
    let again = waiting("task-1", "Claude", 9, Some(30));
    follow_cards(&[], cx);
    follow_cards(std::slice::from_ref(&again), cx);
    let posted = host.notified.take();
    assert_eq!(posted.len(), 1);
    assert_eq!(
        (posted[0].1.as_str(), posted[0].2.as_str()),
        ("Claude needs you", "Sign in to github.com")
    );
    // An agent's own question in its conversation waits as its card does, its words the body.
    let asks = Card {
        status: "Which seat?".into(),
        asks: true,
        ..card("task-2", "Codex", Dot::Waiting)
    };
    follow_cards(&[again, asks], cx);
    let posted = host.notified.take();
    assert_eq!(
        posted
            .iter()
            .map(|(_, title, body)| (title.as_str(), body.as_str()))
            .collect::<Vec<_>>(),
        [("Codex needs you", "Which seat?")]
    );
}

#[gpui::test]
fn a_click_on_a_notification_of_another_run_opens_nothing(cx: &mut TestAppContext) {
    opened(cx);
    assert_eq!(cx.update(|cx| clicked(1, cx)), None);
}
