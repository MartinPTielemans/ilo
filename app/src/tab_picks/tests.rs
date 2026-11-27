use std::cell::RefCell;
use std::rc::Rc;

use gpui::TestAppContext;

use super::{Offer, Offered, Picks, all_checked, entries, not_moved, remove};

fn offered(tab: u64, title: &'static str) -> Offered {
    Offered {
        tab,
        title: title.into(),
        favicon: None,
    }
}

fn offer(space: u64, tabs: &[(u64, &'static str)]) -> Offer {
    Offer {
        space,
        tabs: tabs
            .iter()
            .map(|&(tab, title)| offered(tab, title))
            .collect(),
    }
}

/// A click picks, a second click on the same row unpicks, and the chips stand in the order
/// picked.
#[test]
fn a_click_picks_a_tab_and_a_second_unpicks_it() {
    let mut picks = Picks::default();
    assert!(picks.flip(1, offered(10, "Ten")));
    assert!(picks.flip(1, offered(11, "Eleven")));
    assert_eq!(picks.tabs(), [10, 11]);
    assert!(!picks.flip(1, offered(10, "Ten")));
    assert_eq!(picks.tabs(), [11]);
    assert!(picks.unpick(11));
    assert!(!picks.unpick(11), "a tab not picked was unpicked");
    assert!(picks.is_empty());
}

/// ⌫ in the empty field takes the chip picked last, and nothing with none.
#[test]
fn the_last_chip_goes_first() {
    let mut picks = Picks::default();
    picks.flip(1, offered(10, "Ten"));
    picks.flip(1, offered(11, "Eleven"));
    assert!(picks.unpick_last());
    assert_eq!(picks.tabs(), [10]);
    assert!(picks.unpick_last());
    assert!(!picks.unpick_last());
}

/// The picks are of the Space they were made in: a click in another Space's menu starts over,
/// and a list of another Space keeps none.
#[test]
fn picks_of_another_space_are_not_carried_into_this_one() {
    let mut picks = Picks::default();
    picks.flip(1, offered(10, "Ten"));
    picks.flip(2, offered(20, "Twenty"));
    assert_eq!(picks.tabs(), [20]);
    assert!(picks.keep(&offer(1, &[(20, "Twenty")])));
    assert!(picks.is_empty());
}

/// A chip whose tab is no longer listed goes, silently; one that stays shows its tab as it is
/// now; and a list that changes nothing says so.
#[test]
fn a_chip_goes_with_its_tab_and_follows_its_title() {
    let mut picks = Picks::default();
    picks.flip(1, offered(10, "Ten"));
    picks.flip(1, offered(11, "Eleven"));
    let same = offer(1, &[(10, "Ten"), (11, "Eleven"), (12, "Twelve")]);
    assert!(!picks.keep(&same), "nothing changed");
    assert!(picks.keep(&offer(1, &[(11, "Eleven, again"), (12, "Twelve")])));
    assert_eq!(picks.tabs(), [11]);
    assert_eq!(picks.chips()[0].title.as_ref(), "Eleven, again");
}

/// The send takes the chips' tabs, in their order, for the Space they are of, and leaves no
/// pick: nothing is remembered for the next message.
#[test]
fn the_send_takes_the_chips_tabs_and_leaves_no_pick() {
    let mut picks = Picks::default();
    picks.flip(3, offered(11, "Eleven"));
    picks.flip(3, offered(10, "Ten"));
    let sent = picks.take();
    assert_eq!(sent.tabs(), [11, 10]);
    assert!(picks.is_empty());
    assert!(picks.take().is_empty(), "a pick was sent twice");
}

/// A pick never sent is nothing but the composer's own memory: with no send, there is nothing
/// to hand.
#[test]
fn a_pick_cleared_before_the_send_hands_nothing() {
    let mut picks = Picks::default();
    picks.flip(1, offered(10, "Ten"));
    picks.clear();
    assert!(picks.take().is_empty());
}

/// What a failed test or a debug line prints of a pick names no tab's title.
#[test]
fn no_debug_form_carries_a_tabs_title() {
    let mut picks = Picks::default();
    picks.flip(1, offered(10, "A private title"));
    let listed = offer(1, &[(10, "A private title")]);
    let printed = format!("{picks:?} {listed:?} {:?}", listed.tabs[0]);
    assert!(!printed.contains("private"), "{printed}");
}

/// The second menu lists the offer's tabs by title, in its order, the picked ones checked, and
/// a click on a row asks the composer to flip that tab, the menu staying open with the check
/// it answers.
#[gpui::test]
fn the_menu_lists_the_offer_and_a_click_flips_its_tab(cx: &mut TestAppContext) {
    let flipped: Rc<RefCell<Vec<(u64, u64)>>> = Rc::default();
    let into = flipped.clone();
    let rows = entries(
        &offer(4, &[(10, "Ten"), (11, "Eleven")]),
        &[11],
        move |space, offered, _| {
            into.borrow_mut().push((space, offered.tab));
            true
        },
    );
    let shown: Vec<_> = (rows.iter())
        .map(|row| (row.title.to_string(), row.checked, row.page))
        .collect();
    let expected = [
        ("Ten".to_string(), false, true),
        ("Eleven".to_string(), true, true),
    ];
    assert_eq!(shown, expected);
    let stays = rows[0].stays.clone().expect("a tab's row closes the menu");
    assert!(cx.update(|cx| stays(cx)));
    assert_eq!(*flipped.borrow(), [(4, 10)]);
}

/// The words a screen reader hears are the table's, around the tab's title.
#[test]
fn the_spoken_words_carry_the_tabs_title() {
    assert!(remove("Ten").ends_with("Ten"));
    assert!(not_moved("Ten").starts_with("Ten"));
}

/// All Tabs picks each listed tab not picked yet, after the chips already there, in the menu's
/// order; unpicking takes each listed tab's chip and leaves the rest.
#[test]
fn all_tabs_picks_the_rest_in_order_and_unpicks_them_all() {
    let listed = offer(4, &[(10, "Ten"), (11, "Eleven"), (12, "Twelve")]);
    let mut picks = Picks::default();
    assert!(picks.flip(4, offered(11, "Eleven")));
    picks.set_all(4, &listed.tabs, true);
    assert_eq!(picks.tabs(), [11, 10, 12]);
    picks.set_all(4, &listed.tabs, true);
    assert_eq!(picks.tabs(), [11, 10, 12]);
    picks.set_all(4, &listed.tabs[..2], false);
    assert_eq!(picks.tabs(), [12]);
    picks.set_all(4, &listed.tabs, false);
    assert!(picks.is_empty());
}

/// All Tabs of another Space's menu starts that Space's picks afresh, as a tab's row does.
#[test]
fn all_tabs_of_another_space_drops_this_ones_picks() {
    let mut picks = Picks::default();
    assert!(picks.flip(4, offered(10, "Ten")));
    let there = offer(5, &[(20, "Twenty"), (21, "Twenty-one")]);
    picks.set_all(5, &there.tabs, true);
    assert_eq!(picks.tabs(), [20, 21]);
    assert_eq!(picks.take().tabs(), [20, 21]);
}

/// All Tabs is in the menu with two tabs or more, checked only while each is picked; with one
/// it would be that tab's row again.
#[test]
fn all_tabs_is_listed_with_two_tabs_and_checked_while_each_is_picked() {
    let two = offer(4, &[(10, "Ten"), (11, "Eleven")]);
    assert_eq!(all_checked(&offer(4, &[(10, "Ten")]), &[]), None);
    assert_eq!(all_checked(&two, &[]), Some(false));
    assert_eq!(all_checked(&two, &[11]), Some(false));
    assert_eq!(all_checked(&two, &[11, 10]), Some(true));
}
