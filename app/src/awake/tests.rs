//! The hold, on a host that records what it's asked.

use std::cell::RefCell;

use gpui::TestAppContext;

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<bool>>);

impl Host for Recorder {
    fn hold(&self, on: bool) {
        self.0.borrow_mut().push(on);
    }
}

fn opened(cx: &mut TestAppContext) -> Rc<Recorder> {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx));
    host
}

fn follow_dots(dots: &[Dot], cx: &mut TestAppContext) {
    cx.update(|cx| follow(dots.iter().copied(), cx));
}

#[gpui::test]
fn one_hold_for_every_thread_under_way_let_go_with_the_last(cx: &mut TestAppContext) {
    let host = opened(cx);
    follow_dots(&[], cx);
    follow_dots(&[Dot::Working], cx);
    follow_dots(&[Dot::Working, Dot::Working], cx);
    // One stops: the other still holds it, and the host isn't asked again.
    follow_dots(&[Dot::Ended, Dot::Working], cx);
    // The last is done: let go once.
    follow_dots(&[Dot::Ended, Dot::Done], cx);
    follow_dots(&[Dot::Done], cx);
    assert_eq!(*host.0.borrow(), [true, false]);
}

#[gpui::test]
fn a_thread_waiting_on_the_user_keeps_it_and_an_archived_one_lets_go(cx: &mut TestAppContext) {
    let host = opened(cx);
    follow_dots(&[Dot::Waiting], cx);
    follow_dots(&[Dot::Waiting, Dot::Done], cx);
    // Archived, its card leaves the foot.
    follow_dots(&[Dot::Done], cx);
    assert_eq!(*host.0.borrow(), [true, false]);
}

#[gpui::test]
fn without_a_host_nothing_is_held(cx: &mut TestAppContext) {
    follow_dots(&[Dot::Working], cx);
    cx.update(|cx| assert!(!cx.has_global::<Awake>()));
}
