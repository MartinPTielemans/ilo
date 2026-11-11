//! Which tabs have a hover card and what it has, when it closes, what its buttons do, and when the
//! tabs' pictures are taken, on a host that records what the shell asks.

use std::rc::Rc;

use gpui::{
    Modifiers, MouseButton, Pixels, Point, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext, point, px,
};

use crate::{
    hover_card::{Action, Event},
    icon_picker::{self, Picked, Recents, Subject},
    shell::hover_cards::PICTURE_HIDDEN,
    shell::{Folders, Page, Row, Shell, recorder::Recorder},
    theme_picker,
};

/// A drawn shell with the pinned tab 1, the Today tabs 2, 3 and 4, and the favorite 10, showing 2.
/// Under Reduce Motion, so the rows are in their places from the first frame: their motion runs on
/// real time, and a press at a row's frame drawn mid-slide lands on its neighbour.
fn with_tabs(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    cx.update(|cx| cx.set_reduce_motion(true));
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Work".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            vec![1],
            vec![2, 3, 4],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_favorites(vec![(10, 1)], cx);
        for (tab, title) in [
            (1, "Mail"),
            (2, "Docs"),
            (3, "News"),
            (4, "Maps"),
            (10, "Music"),
        ] {
            shell.set_title(tab, title.into(), cx);
            let url = format!("https://www.{}.example/page", title.to_lowercase());
            shell.set_page(
                tab,
                Page {
                    url: url.into(),
                    ..Default::default()
                },
                cx,
            );
        }
        shell.set_selected(2, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(PICTURE_HIDDEN * 2);
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// What the shell asks of the card as the pointer rests on `tab`'s row or tile.
fn rest_on(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    host: &Recorder,
    tab: u64,
) -> String {
    shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(tab), true, cx));
    cx.run_until_parked();
    let asked = host.take();
    let rests: Vec<_> = asked
        .iter()
        .filter(|call| call.starts_with("card rest"))
        .collect();
    assert_eq!(rests.len(), 1, "{asked:?}");
    rests[0].clone()
}

#[gpui::test]
fn each_kind_of_tab_has_its_own_buttons(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    assert_eq!(
        rest_on(&shell, cx, &host, 3),
        "card rest 3 pin favorite split picture"
    );
    assert_eq!(
        rest_on(&shell, cx, &host, 1),
        "card rest 1 unpin favorite split picture"
    );
    // A favorite can't join a split view, as its drag onto the page can't.
    assert_eq!(
        rest_on(&shell, cx, &host, 10),
        "card rest 10 unfavorite picture"
    );
}

#[gpui::test]
fn the_tab_shown_has_no_picture(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let shown = rest_on(&shell, cx, &host, 2);
    assert!(
        shown.starts_with("card rest 2 ") && shown.ends_with(" none"),
        "{shown}"
    );
}

#[gpui::test]
fn a_split_shown_full_has_no_split_button(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    shell.update(cx, |shell, cx| {
        let split = crate::shell::Split {
            tabs: vec![2, 3, 4],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            vec![1],
            vec![2, 3, 4, 5],
            vec![split],
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    host.take();
    shell.update(cx, |shell, cx| shell.set_title(5, "Five".into(), cx));
    assert_eq!(
        rest_on(&shell, cx, &host, 5),
        "card rest 5 pin favorite picture"
    );
    // And a split view's row has no card.
    shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(2), true, cx));
    assert!(!host.take().iter().any(|call| call.starts_with("card rest")));
}

#[gpui::test]
fn a_private_window_s_cards_only_split(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) =
        cx.add_window_view(|_, cx| Shell::private_window(browser, 9, "Private".into(), 1, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            9,
            Vec::new(),
            vec![20, 21],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_title(20, "One".into(), cx);
        shell.set_title(21, "Two".into(), cx);
        shell.set_selected(20, cx);
    });
    cx.run_until_parked();
    host.take();
    assert_eq!(rest_on(&shell, cx, &host, 21), "card rest 21 split picture");
}

#[gpui::test]
fn the_pointer_leaving_a_row_lets_its_card_go(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    rest_on(&shell, cx, &host, 3);
    shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(3), false, cx));
    assert_eq!(host.take(), ["card left 3"]);
}

#[gpui::test]
fn a_scroll_or_a_press_closes_the_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let frame = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&3]);
    cx.simulate_mouse_move(frame.center(), None, Modifiers::none());
    cx.run_until_parked();
    host.take();
    cx.simulate_event(ScrollEvent {
        position: frame.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-10.))),
        ..Default::default()
    });
    cx.run_until_parked();
    assert!(host.take().contains(&"card close".to_string()), "a scroll");
    cx.simulate_mouse_down(frame.center(), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(host.take().contains(&"card close".to_string()), "a press");
}

/// A middle press closes the row's tab, and its card with it.
#[gpui::test]
fn a_middle_press_closes_the_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let frame = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&3]);
    cx.simulate_mouse_move(frame.center(), None, Modifiers::none());
    cx.run_until_parked();
    host.take();
    cx.simulate_mouse_down(frame.center(), MouseButton::Middle, Modifiers::none());
    cx.run_until_parked();
    let asked = host.take();
    assert!(asked.contains(&"card close".to_string()), "{asked:?}");
    assert!(asked.contains(&"close 3".to_string()), "{asked:?}");
}

#[gpui::test]
fn a_rows_menu_closes_the_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    rest_on(&shell, cx, &host, 3);
    shell.update(cx, |shell, cx| {
        shell.tab_menu(3, point(px(50.), px(200.)), cx)
    });
    assert!(host.take().contains(&"card close".to_string()));
    // And none rests while it's up.
    shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(4), true, cx));
    assert!(!host.take().iter().any(|call| call.starts_with("card rest")));
}

/// No card rests while a name field is open, a tab's or a Space's, its menu up or not, as the card
/// would keep the name being changed (#1452).
#[gpui::test]
fn no_card_rests_while_a_name_field_is_open(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let rests = |cx: &mut VisualTestContext, tab: u64| {
        shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(tab), true, cx));
        host.take().iter().any(|call| call.starts_with("card rest"))
    };
    shell.update(cx, |shell, cx| shell.rename_tab(3, cx));
    assert!(!rests(cx, 4), "a tab's name field is open");
    shell.update_in(cx, |shell, window, cx| shell.cancel_name(window, cx));
    assert!(rests(cx, 4), "the tab's name field closed");
    shell.update(cx, |shell, cx| shell.rename_space(1, cx));
    assert!(!rests(cx, 3), "the Space's name field is open");
}

/// A tab's new title, the name the core reports it was given or its page's, goes to its card,
/// which shows it if it's that tab's (#1452); a title that didn't change doesn't.
#[gpui::test]
fn a_tab_s_new_name_goes_to_its_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    rest_on(&shell, cx, &host, 3);
    let told = |cx: &mut VisualTestContext, title: &str| {
        shell.update(cx, |shell, cx| {
            shell.set_title(3, title.to_string().into(), cx)
        });
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("card changed"))
            .collect::<Vec<_>>()
    };
    assert_eq!(told(cx, "Weather"), ["card changed 3 Weather"]);
    assert!(told(cx, "Weather").is_empty());
}

/// The pickers' windows, which show nowhere.
struct NoWhere;

impl theme_picker::Host for NoWhere {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}

    fn hide(&self) {}

    fn detent(&self) {}
}

impl icon_picker::Host for NoWhere {
    fn show(&self, _: u64, _: f32, _: f32) {}

    fn hide(&self) {}

    fn keep_recents(&self, _: &str) {}
}

/// The theme picker, as Edit Space opens it beside the sidebar, sends the card showing away, and
/// none rests while it or the icon picker shows, as the card would be under it (#1071).
#[gpui::test]
fn a_picker_closes_the_card(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme_picker::open(Rc::new(NoWhere), cx);
        icon_picker::open(Rc::new(NoWhere), Recents::default(), cx);
    });
    let (shell, cx, host) = with_tabs(cx);
    let rests = |cx: &mut VisualTestContext, tab: u64| {
        shell.update(cx, |shell, cx| shell.hover_card(Row::Tab(tab), true, cx));
        host.take().iter().any(|call| call.starts_with("card rest"))
    };
    rest_on(&shell, cx, &host, 3);
    let at = theme_picker::At::Row(point(200., 100.));
    shell.update(cx, |shell, cx| shell.show_theme_picker(1, at, false, cx));
    assert!(host.take().contains(&"card close".to_string()));
    assert!(!rests(cx, 4), "the theme picker shows");
    cx.update(|_, cx| theme_picker::dismissed(cx));
    assert!(rests(cx, 4), "the theme picker closed");

    shell.update(cx, |_, cx| {
        let picked: Picked = Rc::new(|_, _| {});
        icon_picker::show(1, Subject::Tab(3), "".into(), point(0., 0.), picked, cx);
    });
    assert!(!rests(cx, 4), "the icon picker shows");
    cx.update(|_, cx| icon_picker::dismissed(cx));
    assert!(rests(cx, 4), "the icon picker closed");
}

/// Each button makes the call the row's menu makes, or for Split, the one its drag onto the page
/// makes, beside the tab shown.
#[gpui::test]
fn the_buttons_make_the_menus_calls(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let act = |cx: &mut VisualTestContext, tab: u64, action: Action| {
        rest_on(&shell, cx, &host, tab);
        let rest = host.card.borrow().clone().unwrap();
        cx.update(|_, cx| (rest.events)(tab, Event::Act(action), cx));
        cx.run_until_parked();
        host.take_edits()
    };
    assert_eq!(act(cx, 3, Action::Pin), ["pinned 3 true"]);
    assert_eq!(act(cx, 1, Action::Unpin), ["pinned 1 false"]);
    assert_eq!(
        act(cx, 4, Action::Favorite),
        [format!("favorite 4 {}", usize::MAX)]
    );
    assert_eq!(act(cx, 10, Action::Unfavorite), ["move 10 false 0 0"]);
    assert_eq!(act(cx, 3, Action::Split), ["split 3 2 false"]);
}

/// While a page shares another tab, the card of a tab Chrome would offer instead has Share This
/// Tab Instead, which has the core switch the share to it; the others' cards don't.
#[gpui::test]
fn a_tab_to_share_instead_offers_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            share_instead: true,
            ..shell.page(3)
        };
        shell.set_page(3, page, cx);
    });
    assert_eq!(
        rest_on(&shell, cx, &host, 4),
        "card rest 4 pin favorite split picture"
    );
    assert_eq!(
        rest_on(&shell, cx, &host, 3),
        "card rest 3 pin favorite split share-instead picture"
    );
    let rest = host.card.borrow().clone().unwrap();
    cx.update(|_, cx| (rest.events)(3, Event::Act(Action::ShareInstead), cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"share instead 3".to_owned()));
}

/// The sidebar shown over the page while hidden has the cards it has docked, Split and all.
#[gpui::test]
fn a_hidden_sidebars_card_splits(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.sidebar_hidden));
    host.take();
    assert_eq!(
        rest_on(&shell, cx, &host, 3),
        "card rest 3 pin favorite split picture"
    );
    let rest = host.card.borrow().clone().unwrap();
    cx.update(|_, cx| (rest.events)(3, Event::Act(Action::Split), cx));
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["split 3 2 false"]);
}

/// The tabs a run of switches hides are pictured once it's over, never on a switch; the tab shown
/// and an unloaded one aren't.
#[gpui::test]
fn hidden_tabs_are_pictured_once_the_switches_are_quiet(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            unloaded: true,
            ..shell.page(3)
        };
        shell.set_page(3, page, cx);
        shell.set_selected(3, cx);
        shell.set_selected(4, cx);
    });
    cx.run_until_parked();
    let pictured = |host: &Recorder| -> Vec<String> {
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("card picture"))
            .collect()
    };
    assert!(pictured(&host).is_empty(), "nothing on the switch");
    cx.executor().advance_clock(PICTURE_HIDDEN / 2);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.executor().advance_clock(PICTURE_HIDDEN / 2);
    cx.run_until_parked();
    assert!(pictured(&host).is_empty(), "another switch puts it off");
    // Asked once the last switch has been quiet for the Dock dot's 400 ms, while the core may
    // still keep the page it left on screen, which it holds for the picture (#1658).
    cx.executor().advance_clock(PICTURE_HIDDEN / 2);
    cx.run_until_parked();
    assert_eq!(PICTURE_HIDDEN, std::time::Duration::from_millis(400));
    assert_eq!(pictured(&host), ["card picture 2", "card picture 4"]);
}

/// A card showing for a loaded tab in the back pictures it again, as it is now; the tab shown's
/// doesn't.
#[gpui::test]
fn a_card_showing_pictures_its_tab_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    let mut asked = Vec::new();
    for tab in [3, 2] {
        rest_on(&shell, cx, &host, tab);
        let rest = host.card.borrow().clone().unwrap();
        cx.update(|_, cx| (rest.events)(tab, Event::Shown, cx));
        cx.run_until_parked();
        asked.extend(host.take());
    }
    assert_eq!(asked, ["card picture 3"]);
}

/// A drawn shell with the pinned tab 1, then the closed folder 5 holding 6 and the open folder 7,
/// which holds 8, and the open folder 9 holding 10, with the Today tab 2 shown.
fn with_folders(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    let folders = Folders {
        folders: [
            (5, ("Reading".into(), false)),
            (7, ("Rust".into(), true)),
            (9, ("Work".into(), true)),
        ]
        .into(),
        parents: [(6, 5), (7, 5), (8, 7), (10, 9)].into(),
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Work".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            vec![1, 5, 6, 7, 8, 9, 10],
            vec![2],
            Vec::new(),
            folders,
            cx,
        );
        for (tab, title) in [
            (1, "Mail"),
            (2, "Docs"),
            (6, "News"),
            (8, "Book"),
            (10, "Wiki"),
        ] {
            shell.set_title(tab, title.into(), cx);
            let url = format!("https://www.{}.example/", title.to_lowercase());
            shell.set_page(
                tab,
                Page {
                    url: url.into(),
                    ..Default::default()
                },
                cx,
            );
        }
        shell.set_selected(2, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// A point off every row, where the pointer is last seen as it leaves one.
const OFF: Point<Pixels> = point(px(-100.), px(-100.));

/// What the shell asks of the folder preview as the pointer moves onto `row`'s row.
fn preview_on(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, row: u64) {
    shell.update(cx, |shell, cx| {
        let at = shell
            .tip_frames
            .borrow()
            .get(&row)
            .map_or(OFF, |frame| frame.center());
        shell.pointer_to(at, cx);
        shell.folder_preview(Row::Tab(row), true, cx)
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_closed_folder_previews_the_tabs_in_it_and_in_its_folders(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    preview_on(&shell, cx, 5);
    assert_eq!(host.take(), ["folder preview rest 5 6 8"]);
    let titles: Vec<String> = (host.folder_preview.borrow().iter())
        .flat_map(|rest| rest.items.iter().map(|item| item.title.to_string()))
        .collect();
    assert_eq!(
        titles,
        ["News", "Book"],
        "unseen, they keep the sidebar's order"
    );
    let aged = (host.folder_preview.borrow().iter())
        .any(|rest| rest.items.iter().any(|item| !item.age.is_empty()));
    assert!(!aged, "a tab never seen has an age");
    // An open folder's rows show what's in it, and a tab has its card instead.
    preview_on(&shell, cx, 9);
    preview_on(&shell, cx, 1);
    assert!(host.take().is_empty());
    shell.update(cx, |shell, cx| {
        shell.pointer_to(OFF, cx);
        shell.folder_preview(Row::Tab(5), false, cx)
    });
    assert_eq!(host.take(), ["folder preview left 5"]);
}

/// A press that opens a closed folder, with the pointer heard coming onto its row after it, as a
/// click that moves the pointer is, doesn't rest for the preview, which would show over the folder
/// opened, or gone by then.
#[gpui::test]
fn a_press_opening_a_folder_doesnt_rest_for_its_preview(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    shell.update(cx, |shell, cx| shell.press_folder(5, cx));
    preview_on(&shell, cx, 5);
    assert_eq!(host.take(), ["folder open 5 true"]);
}

#[gpui::test]
fn a_preview_lists_the_tab_seen_last_first_with_how_long_ago(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    let (minute, now) = (60_000., super::super::recent_downloads::now());
    host.seen
        .borrow_mut()
        .extend([(6, now - 3. * 24. * 60. * minute), (8, now - 31.5 * minute)]);
    preview_on(&shell, cx, 5);
    assert_eq!(host.take(), ["folder preview rest 5 8 6"]);
    let row = |item: &crate::folder_preview::Item| (item.title.to_string(), item.age.to_string());
    let rows: Vec<(String, String)> = (host.folder_preview.borrow().iter())
        .flat_map(|rest| rest.items.iter().map(row))
        .collect();
    assert_eq!(
        rows,
        [
            ("Book".into(), "31 minutes ago".into()),
            ("News".into(), "3 days ago".into())
        ]
    );
}

/// A hover lost while the preview is open, with the pointer last moved onto the folder's row, isn't
/// the pointer leaving: the browser window loses the hover with no move as the preview takes the
/// keyboard, and GPUI may then say the pointer is wherever the system's cursor is. Its next move
/// off the row is the leave.
#[gpui::test]
fn the_preview_hears_the_pointer_leave_only_once_it_moves_off_the_row(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    preview_on(&shell, cx, 5);
    host.take();
    let row = shell.read_with(cx, |shell, _| shell.tip_frames.borrow()[&5]);
    shell.update(cx, |shell, cx| shell.folder_preview(Row::Tab(5), false, cx));
    assert!(
        host.take().is_empty(),
        "a hover none while the preview was open let it go"
    );
    shell.update(cx, |shell, cx| shell.pointer_to(row.center(), cx));
    assert!(
        host.take().is_empty(),
        "a move on the row let the preview go"
    );
    shell.update(cx, |shell, cx| shell.pointer_to(OFF, cx));
    assert_eq!(host.take(), ["folder preview left 5"]);
    shell.update(cx, |shell, cx| shell.pointer_to(OFF, cx));
    assert!(
        host.take().is_empty(),
        "the preview heard the pointer leave twice"
    );
}

#[gpui::test]
fn the_tab_chosen_in_a_preview_shows_with_its_page_focused(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    preview_on(&shell, cx, 5);
    host.take();
    let rest = host
        .folder_preview
        .borrow()
        .clone()
        .expect("no preview asked for");
    cx.update(|_, cx| (rest.chosen)(8, cx));
    cx.run_until_parked();
    let asked = host.take();
    assert!(asked.contains(&"select 8".to_string()), "{asked:?}");
    assert!(asked.contains(&"focus page".to_string()), "{asked:?}");
}

#[gpui::test]
fn a_press_or_a_menu_ends_the_preview(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_folders(cx);
    shell.update(cx, |shell, cx| shell.end_hovers(cx));
    assert!(host.take().contains(&"folder preview close".to_string()));
    // Nor does one open while a row has its menu up.
    shell.update(cx, |shell, cx| {
        shell.tab_menu(5, point(px(100.), px(200.)), cx)
    });
    host.take();
    preview_on(&shell, cx, 5);
    assert!(
        host.take()
            .iter()
            .all(|call| !call.starts_with("folder preview rest"))
    );
}

/// A hidden sidebar out over the page counts its row's card as its own (#830): the pointer past
/// the panel but on the card keeps it out, back over the panel too, and once it's off both, the
/// sidebar slides away and the card goes with it.
#[gpui::test]
fn a_hidden_sidebar_stays_out_while_the_pointer_is_on_a_row_s_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = with_tabs(cx);
    shell.update(cx, |shell, cx| {
        shell.sidebar_hidden = true;
        shell.set_revealed(true, cx);
    });
    rest_on(&shell, cx, &host, 3);
    let rest = host.card.borrow().clone().unwrap();
    // As the Mac host has it, the card hears a move before the reveal does, in the same turn.
    let pointer = |cx: &mut VisualTestContext, on: bool, x: f32| {
        cx.update(|_, cx| (rest.events)(3, Event::Pointer(on), cx));
        shell.update(cx, |shell, cx| shell.pointer_at(x, cx));
        cx.run_until_parked();
        let closed = host.take().contains(&"card close".to_string());
        (shell.read_with(cx, |shell, _| shell.revealed), closed)
    };
    assert_eq!(
        pointer(cx, true, 600.),
        (true, false),
        "on the card, past the panel"
    );
    assert_eq!(
        pointer(cx, false, 100.),
        (true, false),
        "off the card, back over the panel"
    );
    assert_eq!(pointer(cx, true, 600.), (true, false), "on the card again");
    assert_eq!(
        pointer(cx, false, 700.),
        (false, true),
        "off both, onto the page"
    );
}
