//! Which of the user's tabs the composer offers to hand to a thread, on a host that records what
//! the shell asks of the core.

use std::rc::Rc;

use gpui::{AppContext, TestAppContext};

use crate::shell::{Folders, Page, Shell, recorder::Recorder};

fn space(
    id: u64,
) -> (
    u64,
    gpui::SharedString,
    Option<u32>,
    u64,
    gpui::SharedString,
    gpui::SharedString,
    gpui::SharedString,
) {
    (id, "Space".into(), None, 0, "".into(), "".into(), "".into())
}

fn web(tab: u64) -> Page {
    Page {
        url: format!("https://site{tab}.test/").into(),
        ..Page::default()
    }
}

/// A window on Space 1 with the favourite 5, the pinned tab 7 and today's 10 and 11, each on a
/// page of the web.
fn window(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![space(1)], cx);
        shell.set_favorites(vec![(5, 1)], cx);
        shell.set_tabs(1, vec![7], vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        for tab in [5, 7, 10, 11] {
            shell.set_page(tab, web(tab), cx);
        }
    });
    (shell, host)
}

fn offered(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) -> Vec<u64> {
    let offer = shell.read_with(cx, |shell, cx| shell.tabs_to_offer(None, cx));
    offer.tabs.iter().map(|tab| tab.tab).collect()
}

/// The composer offers today's tabs alone: a favourite and a pinned tab are not offered while
/// one a thread holds has no row or tile to be found by.
#[gpui::test]
fn the_composer_offers_todays_tabs_and_no_favourite_or_pinned_tab(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    assert_eq!(offered(&shell, cx), [10, 11]);
    // The core is asked of those alone, once, for the Space in view.
    assert_eq!(*host.hand_asks.borrow(), [(None, 1, 2)]);
    assert!(shell.read_with(cx, |shell, _| shell.lists_tabs_to_offer()));
}

/// A tab the app would not hand is left out of what the composer offers, not greyed.
#[gpui::test]
fn a_tab_that_cant_be_handed_is_not_offered(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    host.not_to_hand.borrow_mut().push(10);
    assert_eq!(offered(&shell, cx), [11]);
}

/// Nothing is asked of the core for the tabs to hand until the composer asks: not as the
/// window is made, as its tabs are set, on a tab switch or as a tab's page or title changes.
#[gpui::test]
fn nothing_is_asked_before_the_composer_asks(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_selected(11, cx);
        shell.set_title(10, "Ten".into(), cx);
        shell.set_page(11, web(12), cx);
        // What a frame and the chips' watch read is the window's own lists.
        shell.lists_tabs_to_offer();
        shell.tabs_listed();
    });
    assert!(host.hand_asks.borrow().is_empty());
}

/// What the chips are kept against is the window's own: today's tabs on a page of the web, each
/// by its title now. A tab gone to a page of the browser's own, or closed, is not in it.
#[gpui::test]
fn the_chips_list_follows_the_windows_tabs(cx: &mut TestAppContext) {
    let (shell, _) = window(cx);
    shell.update(cx, |shell, cx| shell.set_title(10, "Ten".into(), cx));
    let listed = shell.read_with(cx, |shell, _| shell.tabs_listed());
    assert_eq!(listed.space, 1);
    let tabs: Vec<_> = (listed.tabs.iter())
        .map(|tab| (tab.tab, tab.title.to_string()))
        .collect();
    assert_eq!(tabs[0], (10, "Ten".to_string()));
    assert_eq!(tabs.len(), 2);
    shell.update(cx, |shell, cx| {
        let own = Page {
            url: "ilo://settings/".into(),
            ..Page::default()
        };
        shell.set_page(10, own, cx);
        shell.set_tabs(1, vec![7], vec![10], Vec::new(), Folders::default(), cx);
    });
    let listed = shell.read_with(cx, |shell, _| shell.tabs_listed());
    assert!(listed.tabs.is_empty(), "{listed:?}");
}

/// A private window's composer is offered no tab, and its add menu has no row for them.
#[gpui::test]
fn a_private_window_offers_no_tab(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![90], Vec::new(), Folders::default(), cx);
        shell.set_page(90, web(90), cx);
    });
    assert!(!shell.read_with(cx, |shell, _| shell.lists_tabs_to_offer()));
    assert!(offered(&shell, cx).is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.tabs_listed().tabs.is_empty()));
}
