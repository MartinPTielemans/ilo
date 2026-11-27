//! The kit's choices against a host that records what it's asked: a pop-up and a checkbox
//! drop-down change exactly their one setting, a ⋯ menu's Delete asks first, a form's primary
//! waits for what's typed to be valid.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, AppContext, Pixels, Point, TestAppContext, point, px};

use super::*;
use crate::menu::Closed;

/// What the kit asked of its window, and the answers it waits on.
#[derive(Default)]
struct Recording {
    menus: RefCell<Vec<(Vec<MenuEntry>, Option<menu::Chosen>)>>,
    alerts: RefCell<Vec<(Alert, Option<Box<dyn FnOnce(&mut App)>>)>>,
    opened: RefCell<Vec<String>>,
}

impl Host for Recording {
    fn menu(&self, _: Point<Pixels>, items: Vec<MenuEntry>, chosen: menu::Chosen, _: &mut App) {
        self.menus.borrow_mut().push((items, Some(chosen)));
    }

    fn alert(&self, alert: Alert, confirmed: Box<dyn FnOnce(&mut App)>, _: &mut App) {
        self.alerts.borrow_mut().push((alert, Some(confirmed)));
    }

    fn open(&self, url: &str, _: &mut App) {
        self.opened.borrow_mut().push(url.to_owned());
    }
}

impl Recording {
    /// Closes the last menu shown, as `closed`.
    fn close(&self, closed: Closed, cx: &mut App) {
        let chosen = self
            .menus
            .borrow_mut()
            .last_mut()
            .and_then(|menu| menu.1.take());
        chosen.expect("a menu was shown")(closed, cx);
    }

    /// The last menu's rows' titles, a line as "—".
    fn titles(&self) -> Vec<String> {
        let menus = self.menus.borrow();
        let (items, _) = menus.last().expect("a menu was shown");
        (items.iter())
            .map(|item| match item.title.is_empty() {
                true => "—".to_owned(),
                false => item.title.to_string(),
            })
            .collect()
    }
}

fn at() -> Point<Pixels> {
    point(px(10.), px(20.))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Level {
    Look,
    Work,
    Full,
}

fn levels() -> Vec<Choice<Level>> {
    vec![
        Choice::new(Level::Look, "Look only").icon("icons/eye.svg"),
        Choice::new(Level::Work, "Work on pages").icon("icons/hand.svg"),
        Choice::new(Level::Full, "Full access").icon("icons/lock-open.svg"),
    ]
}

#[gpui::test]
fn a_pop_up_sets_its_one_setting_to_what_was_picked(cx: &mut TestAppContext) {
    let host = Recording::default();
    let set: Rc<RefCell<Vec<Level>>> = Rc::default();
    let closed = Rc::new(RefCell::new(0));
    cx.update(|cx| {
        let (set, closed) = (set.clone(), closed.clone());
        popup::open(
            &host,
            at(),
            levels(),
            Level::Work,
            move |level, _| set.borrow_mut().push(level),
            move |_| *closed.borrow_mut() += 1,
            cx,
        );
    });
    let shown = host.menus.borrow();
    let items = &shown[0].0;
    // The value has the ✓ and opens over the button; each row carries its icon.
    assert_eq!(
        (items.iter().map(|item| item.checked).collect::<Vec<_>>()),
        [false, true, false]
    );
    assert!(items[1].over);
    assert_eq!(items[0].icon, Some("icons/eye.svg"));
    drop(shown);
    cx.update(|cx| host.close(Closed::Chose(2), cx));
    assert_eq!(*set.borrow(), [Level::Full]);
    assert_eq!(*closed.borrow(), 1);
    assert_eq!(popup::label(&levels(), &Level::Full), "Full access");
    assert_eq!(popup::icon(&levels(), &Level::Look), Some("icons/eye.svg"));
}

#[gpui::test]
fn a_pop_up_closed_on_its_value_or_without_a_pick_sets_nothing(cx: &mut TestAppContext) {
    let host = Recording::default();
    let set: Rc<RefCell<Vec<Level>>> = Rc::default();
    for closed in [Closed::Chose(1), Closed::Otherwise, Closed::Left] {
        cx.update(|cx| {
            let set = set.clone();
            let picked = move |level, _: &mut App| set.borrow_mut().push(level);
            popup::open(&host, at(), levels(), Level::Work, picked, |_| {}, cx);
            host.close(closed, cx);
        });
    }
    assert!(set.borrow().is_empty());
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Told {
    NeedsYou,
    Done,
}

fn told() -> Vec<Choice<Told>> {
    vec![
        Choice::new(Told::NeedsYou, "Needs you"),
        Choice::new(Told::Done, "Done"),
    ]
}

#[test]
fn a_checkbox_drop_down_sums_up_its_set() {
    let sum = |on: &[Told]| checks::summary(&told(), on, "Everything", "Nothing");
    assert_eq!(sum(&[Told::NeedsYou, Told::Done]), "Everything");
    assert_eq!(sum(&[Told::Done, Told::NeedsYou]), "Everything");
    assert_eq!(sum(&[Told::Done]), "Done");
    assert_eq!(sum(&[]), "Nothing");
    let three = [told(), vec![Choice::new(Told::Done, "Other")]].concat();
    assert_eq!(
        checks::summary(&three, &[Told::NeedsYou], "Everything", "Nothing"),
        "Needs you"
    );
}

#[gpui::test]
fn a_checkbox_drop_down_flips_exactly_the_member_chosen_and_stays_open(cx: &mut TestAppContext) {
    let host = Recording::default();
    let set: Rc<RefCell<Vec<(Told, bool)>>> = Rc::default();
    cx.update(|cx| {
        let set = set.clone();
        let record = Rc::new(move |told, on, _: &mut App| set.borrow_mut().push((told, on)));
        checks::open(
            &host,
            at(),
            told(),
            vec![Told::NeedsYou, Told::Done],
            record,
            |_| {},
            cx,
        );
    });
    let stays: Vec<_> = (host.menus.borrow()[0].0.iter())
        .map(|item| {
            assert!(item.checked, "both start ticked");
            item.stays.clone().expect("a row that leaves the menu open")
        })
        .collect();
    cx.update(|cx| {
        // Done off, then on again: each time that member alone, and the row's tick follows.
        assert!(!stays[1](cx));
        assert!(stays[1](cx));
        assert!(!stays[0](cx));
    });
    assert_eq!(
        *set.borrow(),
        [
            (Told::Done, false),
            (Told::Done, true),
            (Told::NeedsYou, false)
        ]
    );
}

#[gpui::test]
fn a_row_menus_destructive_items_come_last_after_a_line(cx: &mut TestAppContext) {
    let host: Rc<Recording> = Rc::default();
    let ran: Rc<RefCell<Vec<&str>>> = Rc::default();
    let action = |label: &'static str| {
        let ran = ran.clone();
        row_menu::Action::new(label, move |_| ran.borrow_mut().push(label))
    };
    let delete = action("Delete").destructive(None);
    let actions = vec![delete, action("Open")];
    let (entries, _) = row_menu::items(&actions);
    assert!(entries[2].destructive);
    assert!(!entries[0].destructive);
    cx.update(|cx| row_menu::open(host.clone(), at(), actions, |_| {}, cx));
    assert_eq!(host.titles(), ["Open", "—", "Delete"]);
    // The line is no action: choosing it does nothing.
    cx.update(|cx| host.close(Closed::Chose(1), cx));
    assert!(ran.borrow().is_empty());
    // Only plain ones, or only red ones: no line.
    let (entries, _) = row_menu::items(&[action("Open")]);
    assert_eq!(entries.len(), 1);
    let (entries, _) = row_menu::items(&[action("Delete").destructive(None)]);
    assert_eq!(entries.len(), 1);
}

#[gpui::test]
fn a_row_menus_delete_asks_first_and_does_it_only_on_yes(cx: &mut TestAppContext) {
    let host: Rc<Recording> = Rc::default();
    let deleted = Rc::new(RefCell::new(0));
    let actions = || {
        let deleted = deleted.clone();
        let ask = Alert::new("Delete?", "It can't be undone.", "Delete");
        vec![
            row_menu::Action::new("Open", |_| {}),
            row_menu::Action::new("Delete", move |_| *deleted.borrow_mut() += 1)
                .destructive(Some(ask)),
        ]
    };
    cx.update(|cx| {
        row_menu::open(host.clone(), at(), actions(), |_| {}, cx);
        host.close(Closed::Chose(2), cx);
    });
    // Asked, and nothing deleted while it asks.
    assert_eq!(host.alerts.borrow().len(), 1);
    assert_eq!(host.alerts.borrow()[0].0.action, "Delete");
    assert_eq!(*deleted.borrow(), 0);
    // Cancelled: the answer never comes, and nothing is deleted.
    host.alerts.borrow_mut().clear();
    assert_eq!(*deleted.borrow(), 0);
    // Asked again and confirmed: deleted once.
    cx.update(|cx| {
        row_menu::open(host.clone(), at(), actions(), |_| {}, cx);
        host.close(Closed::Chose(2), cx);
    });
    let confirmed = host.alerts.borrow_mut()[0].1.take().expect("an answer");
    cx.update(confirmed);
    assert_eq!(*deleted.borrow(), 1);
}

#[gpui::test]
fn an_action_that_asks_nothing_is_done_at_once(cx: &mut TestAppContext) {
    let host: Rc<dyn Host> = Rc::new(Recording::default());
    let opened = Rc::new(RefCell::new(0));
    let open = {
        let opened = opened.clone();
        row_menu::Action::new("Open", move |_| *opened.borrow_mut() += 1)
    };
    cx.update(|cx| row_menu::choose(&host, &open, cx));
    assert_eq!(*opened.borrow(), 1);
}

#[gpui::test]
fn a_forms_primary_waits_until_what_is_typed_is_valid(cx: &mut TestAppContext) {
    let added: Rc<RefCell<Vec<String>>> = Rc::default();
    let field = cx.update(|cx| cx.new(|cx| crate::text_input::TextInput::new("Site", cx)));
    let form = {
        let added = added.clone();
        form::Form::popover(at(), field.clone(), "Add", move |site, _| {
            added.borrow_mut().push(site)
        })
    };
    cx.update(|cx| {
        assert!(!form.ready(cx));
        assert!(
            !form.submit(cx),
            "an empty field adds nothing and stays open"
        );
    });
    let typed = cx.update(|cx| {
        cx.new(|cx| crate::text_input::TextInput::new("Site", cx).with_text("  ".into()))
    });
    let blank = form::Form {
        fields: vec![typed],
        ..form::Form::popover(at(), field.clone(), "Add", |_, _| {})
    };
    cx.update(|cx| assert!(!blank.ready(cx), "spaces are nothing"));
    let typed = cx.update(|cx| {
        cx.new(|cx| crate::text_input::TextInput::new("Site", cx).with_text("example.com".into()))
    });
    let form = form::Form {
        fields: vec![typed],
        ..form
    };
    cx.update(|cx| {
        assert!(form.ready(cx));
        assert!(form.submit(cx));
    });
    assert_eq!(*added.borrow(), ["example.com"]);
}

#[gpui::test]
fn a_form_that_only_tells_has_no_primary_to_choose(cx: &mut TestAppContext) {
    let form = form::Form {
        title: "How".into(),
        body: Some("Words.".into()),
        fields: Vec::new(),
        primary: None,
        cancel: "Cancel".into(),
        valid: Rc::new(|_: &[String]| true),
        done: Rc::new(|_, _| panic!("nothing to do")),
        placement: form::Placement::Dialog,
    };
    cx.update(|cx| {
        assert!(!form.ready(cx));
        assert!(!form.submit(cx));
    });
}

#[test]
fn arrows_move_the_tiles_pick_and_stop_at_the_ends() {
    assert_eq!(tiles::moved("right", 0, 3), Some(1));
    assert_eq!(tiles::moved("left", 1, 3), Some(0));
    assert_eq!(tiles::moved("left", 0, 3), None);
    assert_eq!(tiles::moved("right", 2, 3), None);
    assert_eq!(tiles::moved("up", 1, 3), None);
}

#[test]
fn help_shows_the_pointers_else_the_keyboards() {
    let helps = help::Helps::default();
    let (a, b): (gpui::SharedString, gpui::SharedString) = ("a".into(), "b".into());
    assert_eq!(helps.shown(), None);
    helps.hover(&a, true);
    assert_eq!(helps.shown(), Some(a.clone()));
    // Leaving another one's doesn't take this one's away.
    helps.hover(&b, false);
    assert_eq!(helps.shown(), Some(a.clone()));
    helps.hover(&a, false);
    assert_eq!(helps.shown(), None);
}

#[test]
fn red_is_the_menus_destructive_ink_in_each_look() {
    let dark = crate::settings_rows::Look::settings(true);
    let light = crate::settings_rows::Look::settings(false);
    assert_eq!(
        red(&dark),
        gpui::rgba(crate::tokens::menu::DARK_DESTRUCTIVE)
    );
    assert_eq!(
        red(&light),
        gpui::rgba(crate::tokens::menu::LIGHT_DESTRUCTIVE)
    );
}
