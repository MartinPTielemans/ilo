use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Context, Modifiers, Pixels, Point, TestAppContext, Window, div, prelude::*};

use super::{Look, Plus, Row};
use crate::menu::{Closed, EntriesMenu};
use crate::shell::MenuEntry;
use crate::text_input::Chosen;

/// A field drawing a plus over the rows `rows` gives it each frame.
struct Field {
    plus: Plus,
    rows: Rc<dyn Fn() -> Vec<Row>>,
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = Look {
            side: 24.,
            radius: 6.,
            glyph: 16.,
            ink: gpui::black(),
            open: gpui::white(),
            off: 0.26,
            drop: 4.,
            a11y: false,
        };
        let plus = self.plus.draw("plus", (self.rows)(), &look, cx.entity_id());
        div().size_full().p_4().child(plus)
    }
}

/// A menu the plus showed: its rows' titles, where it hangs, and what closes it.
type Shown = Rc<RefCell<Option<(Vec<String>, Point<Pixels>, Chosen)>>>;

/// A window with a field whose rows are named `names`, each pick of one recorded in `picked`,
/// and the menu it shows caught in what it returns.
fn field<'a>(
    cx: &'a mut TestAppContext,
    names: &'static [&'static str],
    picked: Rc<RefCell<Vec<&'static str>>>,
) -> (Plus, &'a mut gpui::VisualTestContext, Shown) {
    let shown: Shown = Rc::default();
    let into = shown.clone();
    cx.update(|cx| {
        cx.set_global(EntriesMenu(Rc::new(move |items, at, _, _, chosen, _| {
            let titles = items.iter().map(|item| item.title.to_string()).collect();
            *into.borrow_mut() = Some((titles, at, chosen));
        })))
    });
    let plus = Plus::default();
    let drawn = plus.clone();
    let rows = Rc::new(move || {
        (names.iter())
            .map(|&name| {
                let picked = picked.clone();
                Row::new(MenuEntry::named(name, true), move |_, _| {
                    picked.borrow_mut().push(name)
                })
            })
            .collect()
    });
    let (_, cx) = cx.add_window_view(move |_, _| Field { plus: drawn, rows });
    cx.run_until_parked();
    (plus, cx, shown)
}

/// With no row the plus stays in its place, disabled: drawn, but a press and the keys a button
/// takes show no menu.
#[gpui::test]
fn a_plus_with_no_row_is_drawn_disabled_and_takes_no_press_or_key(cx: &mut TestAppContext) {
    let (plus, cx, shown) = field(cx, &[], Rc::default());
    let drawn = plus
        .drawn()
        .expect("a plus with nothing to add left its place");
    cx.simulate_click(drawn.center(), Modifiers::none());
    cx.simulate_keystrokes("enter space");
    assert!(shown.borrow().is_none(), "a menu with nothing to add");
}

/// A click on the plus shows the caller's rows, in its order, under it; a pick runs its row
/// alone, once.
#[gpui::test]
fn a_plus_shows_the_callers_rows_and_a_pick_runs_that_row(cx: &mut TestAppContext) {
    let picked: Rc<RefCell<Vec<&str>>> = Rc::default();
    let (plus, cx, shown) = field(cx, &["File or Folder", "Second"], picked.clone());
    let drawn = plus.drawn().expect("a plus with rows wasn't drawn");
    cx.simulate_click(drawn.center(), Modifiers::none());
    let (titles, at, chosen) = shown.borrow_mut().take().expect("the plus showed no menu");
    assert_eq!(titles, ["File or Folder", "Second"]);
    assert_eq!(at, gpui::point(drawn.left(), drawn.bottom() + gpui::px(4.)));
    cx.update(|window, cx| chosen(Closed::Chose(1), window, cx));
    assert_eq!(*picked.borrow(), ["Second"]);
}

/// A menu closed without a pick runs no row.
#[gpui::test]
fn a_plus_menu_closed_without_a_pick_runs_nothing(cx: &mut TestAppContext) {
    let picked: Rc<RefCell<Vec<&str>>> = Rc::default();
    let (plus, cx, shown) = field(cx, &["File or Folder"], picked.clone());
    cx.simulate_click(plus.drawn().unwrap().center(), Modifiers::none());
    let (_, _, chosen) = shown.borrow_mut().take().expect("the plus showed no menu");
    cx.update(|window, cx| chosen(Closed::Otherwise, window, cx));
    assert!(picked.borrow().is_empty(), "a row ran without a pick");
}

/// A second menu's row that picks, as the user's tabs' All Tabs, closes the menu and runs once
/// it's chosen; one that stays runs nothing at the close, as its entry did its own.
#[gpui::test]
fn a_second_menus_row_that_picks_runs_at_the_close(cx: &mut TestAppContext) {
    let shown: Shown = Rc::default();
    let into = shown.clone();
    cx.update(|cx| {
        cx.set_global(EntriesMenu(Rc::new(move |items, at, _, _, chosen, _| {
            let titles = items.iter().map(|item| item.title.to_string()).collect();
            *into.borrow_mut() = Some((titles, at, chosen));
        })))
    });
    let picked: Rc<RefCell<Vec<&str>>> = Rc::default();
    let into = picked.clone();
    let rows = Rc::new(move || {
        let into = into.clone();
        let under = move |_: &mut gpui::App| {
            let all = Row::new(MenuEntry::named("All", true), move |_, _| {
                into.borrow_mut().push("All")
            });
            let one = MenuEntry::named("One", true).stays(|_| true);
            vec![all, Row::staying(one)]
        };
        vec![Row::over(MenuEntry::named("Tabs", true), under)]
    });
    let plus = Plus::default();
    let drawn = plus.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Field { plus: drawn, rows });
    cx.run_until_parked();
    cx.simulate_click(plus.drawn().unwrap().center(), Modifiers::none());
    let (titles, _, chosen) = shown.borrow_mut().take().expect("the plus showed no menu");
    assert_eq!(titles, ["Tabs", "All", "One"]);
    cx.update(|window, cx| chosen(Closed::Chose(2), window, cx));
    assert!(picked.borrow().is_empty(), "a staying row ran at close");
    cx.simulate_click(plus.drawn().unwrap().center(), Modifiers::none());
    let (_, _, chosen) = shown.borrow_mut().take().expect("the plus showed no menu");
    cx.update(|window, cx| chosen(Closed::Chose(1), window, cx));
    assert_eq!(*picked.borrow(), ["All"]);
}
