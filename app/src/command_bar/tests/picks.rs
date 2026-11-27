//! The user's tabs picked in the bar's composer: the add menu's row for them, their chips, and
//! what the thread's start is given, on the host that records what the bar asks.

use super::*;
use crate::tab_picks::{ALL_TABS, YOUR_TABS};

/// The composer open on what's typed, in a window that lists `tabs` for it, each by its title.
fn listing(cx: &mut TestAppContext, tabs: &[(u64, &'static str)]) -> Bar {
    let mut bar = composing(cx, false);
    *bar.host.listed.borrow_mut() = tabs.to_vec();
    bar.redraw();
    bar
}

impl Bar {
    fn redraw(&mut self) {
        (self.window)
            .update(&mut self.cx.cx, |_, _, cx| cx.notify())
            .unwrap();
        self.cx.run_until_parked();
    }

    /// The add menu's rows as a click on the plus shows them; None where it shows no menu.
    fn add_menu(&mut self) -> Option<Vec<MenuEntry>> {
        let shown: Rc<RefCell<Option<Vec<MenuEntry>>>> = Rc::default();
        let into = shown.clone();
        self.cx.update(|_, cx| {
            cx.set_global(crate::menu::EntriesMenu(Rc::new(
                move |items, _, _, _, _, _| *into.borrow_mut() = Some(items),
            )))
        });
        self.cx.run_until_parked();
        (self.window)
            .update(&mut self.cx.cx, |bar, window, cx| {
                bar.open_add_menu_for_test(window, cx)
            })
            .unwrap();
        shown.borrow_mut().take()
    }

    /// The user's click on the row titled `title` of the add menu, opened for it. Whether the
    /// row shows its check after it.
    fn click_tab(&mut self, title: &str) -> bool {
        let rows = self.add_menu().expect("the plus showed no menu");
        let row = (rows.iter().find(|row| row.title.to_string() == title)).expect("no such row");
        let stays = row.stays.clone().expect("a tab's row closes the menu");
        let checked = self.cx.update(|_, cx| stays(cx));
        self.cx.run_until_parked();
        checked
    }

    /// The tabs the composer's chips stand for, in their order.
    fn chips(&mut self) -> Vec<u64> {
        (self.window)
            .read_with(&self.cx.cx, |bar, _| {
                let ask = bar.ask.as_ref();
                ask.map(|ask| ask.picks.tabs()).unwrap_or_default()
            })
            .unwrap()
    }

    /// The window's tabs are `tabs` now, which the bar hears of if it watches them.
    fn tabs_now(&mut self, tabs: &[(u64, &'static str)]) {
        *self.host.listed.borrow_mut() = tabs.to_vec();
        let listed = self.host.tabs_now();
        // Held by itself while it's called: hearing of a tab gone, the bar may end the watch.
        let watch = self.host.watch.borrow().clone();
        self.cx.update(|_, cx| {
            if let Some(then) = watch {
                then(listed, cx)
            }
        });
        self.cx.run_until_parked();
    }

    fn empty_the_field(&mut self) {
        self.input_method(|input, window, cx| input.set_text("".into(), 0, window, cx));
    }
}

/// The add menu's row for the user's tabs opens a second menu of them, by title, in the
/// window's order, after All Tabs (Z33a), spoken with how many; a click picks one, which is a
/// chip, the row checked the next time, and All Tabs not while one is unpicked; a second click
/// unpicks it.
#[gpui::test]
fn the_add_menu_lists_the_users_tabs_and_a_click_picks_one(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    let rows = bar.add_menu().expect("the plus showed no menu");
    let shown: Vec<_> = (rows.iter())
        .map(|row| (row.title.to_string(), row.parent, row.checked))
        .collect();
    let expected = [
        (YOUR_TABS.to_string(), None, false),
        (ALL_TABS.to_string(), Some(0), false),
        ("Ten".to_string(), Some(0), false),
        ("Eleven".to_string(), Some(0), false),
    ];
    assert_eq!(shown, expected);
    assert_eq!(rows[1].spoken.as_deref(), Some("All Tabs, 2 tabs"));
    assert!(bar.click_tab("Eleven"));
    assert_eq!(bar.chips(), [11]);
    let rows = bar.add_menu().unwrap();
    assert!(
        rows[3].checked && !rows[2].checked && !rows[1].checked,
        "the pick isn't checked"
    );
    assert!(!bar.click_tab("Eleven"));
    assert!(bar.chips().is_empty());
}

/// The plus's own click opens that menu: the tabs are asked for from the click's handler, with
/// the bar in no update of its own, once.
#[gpui::test]
fn a_click_on_the_plus_lists_the_users_tabs(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten")]);
    let shown: Rc<RefCell<Option<Vec<MenuEntry>>>> = Rc::default();
    let into = shown.clone();
    bar.cx.update(|_, cx| {
        cx.set_global(crate::menu::EntriesMenu(Rc::new(
            move |items, _, _, _, _, _| *into.borrow_mut() = Some(items),
        )))
    });
    // The window as tall as the bar asked the host to make it, so the plus is in it.
    let asked = bar.asked_height();
    bar.cx.simulate_resize(window(WIDTH, asked));
    bar.cx.run_until_parked();
    let plus = (bar.window)
        .read_with(&bar.cx.cx, |bar, _| bar.plus.drawn())
        .unwrap()
        .expect("no plus drawn");
    bar.cx.simulate_click(plus.center(), Modifiers::none());
    bar.cx.run_until_parked();
    let rows = shown.borrow_mut().take().expect("the click showed no menu");
    let titles: Vec<_> = rows.iter().map(|row| row.title.to_string()).collect();
    assert_eq!(titles, [YOUR_TABS, "Ten"]);
    assert_eq!(bar.host.hand_asks.get(), 1);
}

/// A tab the app would not hand is left out of the menu, not greyed; with none to list the
/// row is not in the menu, and where the window lists no tab at all the plus has no row for
/// them.
#[gpui::test]
fn a_tab_that_cant_be_handed_isnt_listed(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    bar.host.unfit.borrow_mut().push(10);
    let rows = bar.add_menu().expect("the plus showed no menu");
    let titles: Vec<_> = rows.iter().map(|row| row.title.to_string()).collect();
    assert_eq!(titles, [YOUR_TABS, "Eleven"]);
    bar.host.unfit.borrow_mut().push(11);
    assert!(bar.add_menu().is_none(), "a row with no tab under it");
    let mut bar = listing(cx, &[]);
    assert!(bar.add_menu().is_none());
    assert_eq!(bar.host.hand_asks.get(), 0, "asked with no tab listed");
}

/// Which tabs may be handed is asked once each time the add menu opens, and at no other time:
/// not as the bar opens, per key, as the composer opens, on a pick, as a chip goes or at the
/// send.
#[gpui::test]
fn the_tabs_are_asked_for_as_the_add_menu_opens_and_then_alone(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    bar.type_text(" and back");
    bar.keys("left right");
    assert_eq!(bar.host.hand_asks.get(), 0);
    bar.add_menu();
    assert_eq!(bar.host.hand_asks.get(), 1);
    bar.click_tab("Ten");
    assert_eq!(bar.host.hand_asks.get(), 2, "one ask per menu opened");
    bar.type_text("!");
    bar.tabs_now(&[(11, "Eleven")]);
    assert!(bar.chips().is_empty());
    bar.keys("enter");
    assert_eq!(bar.host.hand_asks.get(), 2);
}

/// The send carries the chips' tabs, in the order picked, and the composer opened again holds
/// none: nothing is remembered for the next thread.
#[gpui::test]
fn the_start_is_given_the_chips_tabs_and_the_next_composer_none(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    bar.click_tab("Eleven");
    bar.click_tab("Ten");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["task claude pack for the trip tabs [11, 10]"]);
    let mut bar = listing(cx, &[(10, "Ten")]);
    assert!(bar.chips().is_empty());
    bar.keys("enter");
    assert_eq!(bar.calls(), ["task claude pack for the trip"]);
}

/// A pick is nothing until its send: back to search, the picks go with the composer, and the
/// thread then started is handed nothing. Words alone never pick a tab, whatever they name.
#[gpui::test]
fn a_pick_never_sent_hands_nothing_and_typed_text_picks_nothing(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten")]);
    bar.click_tab("Ten");
    bar.keys("tab tab");
    assert!(bar.chips().is_empty(), "the picks outlived their composer");
    bar.type_text(" Ten 10 tab:10");
    bar.keys("enter");
    let started: Vec<_> = (bar.calls().into_iter())
        .filter(|call| call.starts_with("task "))
        .collect();
    assert_eq!(started.len(), 1, "{started:?}");
    assert!(started[0].ends_with("Ten 10 tab:10"), "{started:?}");
}

/// With a chip and no words Start and ↩ are live and the thread starts on the tabs alone; with
/// neither, nothing starts.
#[gpui::test]
fn a_chip_and_no_words_starts_the_thread(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten")]);
    bar.empty_the_field();
    bar.keys("enter");
    assert!(bar.calls().is_empty(), "a thread of nothing started");
    bar.click_tab("Ten");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["task claude  tabs [10]"]);
}

/// ⌫ in the empty field takes the last chip; with words before the caret it is the field's,
/// and during a Japanese composition the input method's.
#[gpui::test]
fn backspace_in_the_empty_field_takes_the_last_chip(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    bar.click_tab("Ten");
    bar.click_tab("Eleven");
    bar.keys("backspace");
    assert_eq!(bar.chips(), [10, 11], "⌫ after words took a chip");
    bar.empty_the_field();
    bar.compose("に");
    assert!(bar.composing());
    bar.keys("backspace");
    assert_eq!(bar.chips(), [10, 11], "⌫ in a composition took a chip");
    bar.empty_the_field();
    bar.keys("backspace");
    assert_eq!(bar.chips(), [10]);
    bar.keys("backspace");
    assert!(bar.chips().is_empty());
}

/// A picked tab that closes, or leaves the pages the composer lists, loses its chip, silently;
/// one that stays shows its tab as it is now. The window's tabs are watched only while a chip
/// is held.
#[gpui::test]
fn a_chip_follows_its_tab_and_goes_with_it(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten"), (11, "Eleven")]);
    assert!(bar.host.watch.borrow().is_none(), "watched with no chip");
    bar.click_tab("Ten");
    bar.click_tab("Eleven");
    assert!(bar.host.watch.borrow().is_some(), "a chip nobody keeps");
    bar.tabs_now(&[(11, "Eleven, later")]);
    assert_eq!(bar.chips(), [11]);
    let title = (bar.window)
        .read_with(&bar.cx.cx, |bar, _| {
            bar.ask.as_ref().unwrap().picks.chips()[0].title.to_string()
        })
        .unwrap();
    assert_eq!(title, "Eleven, later");
    (bar.window)
        .update(&mut bar.cx.cx, |bar, _, cx| bar.unpick(11, cx))
        .unwrap();
    bar.cx.run_until_parked();
    assert!(bar.host.watch.borrow().is_none(), "watched after its chip");
    bar.keys("enter");
    assert_eq!(bar.calls(), ["task claude pack for the trip"]);
}

/// The bar is as tall as what it draws: a chip's row over the field makes it taller by that
/// row, and it is as before once the chip is gone.
#[gpui::test]
fn the_bar_grows_by_its_chips_row(cx: &mut TestAppContext) {
    let mut bar = listing(cx, &[(10, "Ten")]);
    let height = |bar: &mut Bar| {
        (bar.window)
            .read_with(&bar.cx.cx, |bar, _| bar.drawn_height())
            .unwrap()
    };
    let bare = height(&mut bar);
    bar.click_tab("Ten");
    let chipped = height(&mut bar);
    assert!(chipped > bare, "{chipped} with a chip, {bare} without");
    assert_eq!(bar.host.heights.borrow().last(), Some(&chipped));
    bar.empty_the_field();
    bar.keys("backspace");
    assert_eq!(height(&mut bar), bare);
    assert_eq!(bar.host.heights.borrow().last(), Some(&bare));
}
