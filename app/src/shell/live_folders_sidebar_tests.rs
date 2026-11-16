//! A Live Folder in the sidebar (#2979): the Space's menu makes it and takes it away, what the
//! user does asks for its lists and nothing else does, and its rows sit under the tabs opened from
//! it, a press opening one in the folder.

use std::{rc::Rc, time::Duration};

use gpui::{Entity, TestAppContext, VisualTestContext, point, px};

use super::{LiveAnswer, LiveItem, LiveList, LiveSource, PullState};
use crate::shell::{Folders, ROW_PITCH, Shell, Slot, commands::bind_keys, recorder::Recorder};
use crate::tokens::shell::live_folder::HEADING_HEIGHT;

const FOLDER: u64 = 20;

fn item(list: LiveList, number: u32) -> LiveItem {
    LiveItem {
        list,
        repo: "harbor/web".into(),
        number,
        title: format!("Change {number}").into(),
        state: PullState::Open,
    }
}

/// The user's Space 1, showing Today's tab 3, beside an agent's Space 2, under Reduce Motion.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        bind_keys(cx);
        cx.set_reduce_motion(true);
    });
    window_on(cx, Rc::new(Recorder::default()))
}

/// The window of `window` on `host`, which another window may share, as the core is.
fn window_on(
    cx: &mut TestAppContext,
    host: Rc<Recorder>,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        let other = (2, "Other".into(), None, 1, "".into(), "".into(), "".into());
        let agent = (
            9,
            "Agent".into(),
            None,
            1,
            "".into(),
            "claude".into(),
            "".into(),
        );
        shell.set_spaces(vec![space, other, agent], cx);
        shell.set_space(1, cx);
        shell.set_selected(3, cx);
        shell.set_tabs(1, vec![1], vec![3], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![5], Vec::new(), Folders::default(), cx);
    });
    draw(&shell, cx);
    cx.update(|window, _| window.activate_window());
    draw(&shell, cx);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    host.take();
    host.take_edits();
    (shell, cx, host)
}

fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// The core's report of Space 1 with the folder after pinned tab 1, open or not, holding `tabs`.
fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, open: bool, tabs: &[u64]) {
    shell.update(cx, |shell, cx| {
        let mut folders = Folders::default();
        folders
            .folders
            .insert(FOLDER, ("Pull Requests".into(), open));
        folders.live.insert(FOLDER, LiveSource::PullRequests);
        for &tab in tabs {
            folders.parents.insert(tab, FOLDER);
        }
        let pinned = [1, FOLDER].iter().chain(tabs).copied().collect();
        shell.set_tabs(1, pinned, vec![3], Vec::new(), folders, cx);
    });
    draw(shell, cx);
}

/// Now, in milliseconds since the Unix epoch, as the core gives an answer's time.
fn just_now() -> i64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.unwrap().as_millis() as i64
}

fn fetches(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls
        .filter(|call| call.starts_with("fetch live folder"))
        .collect()
}

fn menu(host: &Recorder) -> Vec<String> {
    let menus = host.menus.borrow();
    let items = menus.last().into_iter().flatten();
    items.map(|(title, _)| title.to_string()).collect()
}

#[gpui::test]
fn the_spaces_menu_makes_one_and_unticking_deletes_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(10.), px(10.)), cx)
    });
    let titles = menu(&host);
    let at = |title: &str| titles.iter().position(|each| each == title).unwrap();
    assert!(at("New Folder…") < at("Live Folders"));
    assert!(!host.entries.borrow()[at("GitHub Pull Requests")].checked);
    cx.update(|_, cx| host.choose("GitHub Pull Requests", cx));
    assert_eq!(
        host.take_edits(),
        ["live folder 1001 1 PullRequests Pull Requests"]
    );
    // The core reports it, and it asks for its lists at once.
    report(&shell, cx, true, &[]);
    assert_eq!(
        fetches(&host),
        [format!(
            "fetch live folder {FOLDER} PullRequests refresh false"
        )]
    );
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(10.), px(10.)), cx)
    });
    let titles = menu(&host);
    let ticked = titles
        .iter()
        .position(|each| each == "GitHub Pull Requests");
    assert!(host.entries.borrow()[ticked.unwrap()].checked);
    cx.update(|_, cx| host.choose("GitHub Pull Requests", cx));
    assert!(
        host.take_edits()
            .iter()
            .any(|edit| edit.contains(&FOLDER.to_string()))
    );
}

#[gpui::test]
fn a_folder_made_asks_once_in_each_window_and_isnt_a_refresh(cx: &mut TestAppContext) {
    let (shell, first, host) = window(cx);
    report(&shell, first, false, &[]);
    let once = vec![format!(
        "fetch live folder {FOLDER} PullRequests refresh false"
    )];
    assert_eq!(fetches(&host), once);
    // Drawn again, or reported again unchanged, it doesn't ask again.
    report(&shell, first, false, &[]);
    draw(&shell, first);
    assert!(fetches(&host).is_empty());
    // Another window hears the same report from the core, and asks once too; the core joins the
    // two, or answers the second from what it kept.
    let (other, second, _) = window_on(cx, host.clone());
    report(&other, second, false, &[]);
    assert_eq!(fetches(&host), once);
}

#[gpui::test]
fn an_agents_space_has_none(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.space_menu(9, point(px(10.), px(10.)), cx)
    });
    assert!(!menu(&host).iter().any(|title| title == "Live Folders"));
    shell.update(cx, |shell, cx| {
        assert!(!shell.offers_live_folders(9));
        shell.toggle_live_folder(9, LiveSource::PullRequests, cx);
    });
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn only_what_the_user_does_asks_and_only_refresh_skips_the_cores_two_minutes(
    cx: &mut TestAppContext,
) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, false, &[]);
    assert_eq!(fetches(&host).len(), 1);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(
            FOLDER,
            LiveAnswer::Items(vec![item(LiveList::Created, 7)]),
            just_now(),
            cx,
        )
    });
    // Drawn again and again, nothing asks.
    draw(&shell, cx);
    draw(&shell, cx);
    assert!(fetches(&host).is_empty());
    // The window coming back, the folder opening and a page of the site ask, and the core answers
    // what it has from inside its two minutes.
    cx.deactivate_window();
    draw(&shell, cx);
    cx.update(|window, _| window.activate_window());
    draw(&shell, cx);
    // The window's ask is made once, when the frame that saw it active is drawn.
    assert_eq!(fetches(&host).len(), 1);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    draw(&shell, cx);
    assert!(fetches(&host).is_empty());
    report(&shell, cx, true, &[]);
    shell.update(cx, |shell, cx| {
        shell.live_page_committed(3, "https://github.com/harbor/web/pull/7", cx)
    });
    let asked = fetches(&host);
    assert_eq!(
        asked,
        vec![format!("fetch live folder {FOLDER} PullRequests refresh false"); 2]
    );
    // Refresh asks the site inside them.
    shell.update(cx, |shell, cx| {
        shell.tab_menu(FOLDER, point(px(10.), px(10.)), cx)
    });
    assert_eq!(
        menu(&host),
        [
            "Refresh",
            "",
            "Rename Folder",
            "Convert to Folder",
            "",
            "Delete Folder"
        ]
    );
    cx.update(|_, cx| host.choose("Refresh", cx));
    assert_eq!(
        fetches(&host),
        [format!(
            "fetch live folder {FOLDER} PullRequests refresh true"
        )]
    );
}

#[gpui::test]
fn a_space_shown_asks_once_its_switch_is_drawn(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Failed, just_now(), cx);
        shell.set_space(2, cx);
    });
    host.take();
    // Another Space's folders don't ask.
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(&shell, cx);
    draw(&shell, cx);
    cx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(fetches(&host).is_empty());
    shell.update(cx, |shell, cx| {
        shell.set_space(1, cx);
        // Not in the update that shows it.
        assert!(
            !host
                .take()
                .iter()
                .any(|call| call.starts_with("fetch live"))
        );
        let _ = cx;
    });
    // Its slide drawn to its end, the fetch waits for the frame after.
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(&shell, cx);
    draw(&shell, cx);
    assert!(fetches(&host).is_empty());
    cx.update(|window, cx| window.simulate_next_frame(cx));
    assert_eq!(
        fetches(&host),
        [format!(
            "fetch live folder {FOLDER} PullRequests refresh false"
        )]
    );
}

#[gpui::test]
fn a_pressed_row_opens_its_tab_in_its_place(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    let items = vec![
        item(LiveList::ReviewRequests, 3),
        item(LiveList::Created, 7),
        item(LiveList::Created, 8),
    ];
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Items(items.clone()), just_now(), cx)
    });
    draw(&shell, cx);
    let room = 2. * HEADING_HEIGHT + 3. * ROW_PITCH;
    // Review requests' heading and #3, Created by me's heading, then #7.
    let seven = shell.read_with(cx, |shell, _| {
        let layout = shell.layout();
        let live = layout[&Slot::Live(FOLDER, 0)];
        assert_eq!(live, layout[&Slot::Tab(FOLDER)] + ROW_PITCH);
        assert_eq!(layout[&Slot::Divider], live + room);
        live + 2. * HEADING_HEIGHT + ROW_PITCH
    });
    host.take();
    shell.update(cx, |shell, cx| {
        shell.open_live_item(FOLDER, items[1].url(), cx)
    });
    assert_eq!(
        host.take(),
        [format!(
            "open https://github.com/harbor/web/pull/7 true {FOLDER} 0"
        )]
    );
    // Opened, its tab's row is where its row was, and nothing else moves.
    host.pinned_urls.borrow_mut().insert(30, items[1].url());
    report(&shell, cx, true, &[30]);
    shell.read_with(cx, |shell, _| {
        let layout = shell.layout();
        assert_eq!(layout[&Slot::Tab(30)], seven);
        assert_eq!(layout[&Slot::Live(FOLDER, 1)], seven + ROW_PITCH);
        assert_eq!(
            layout[&Slot::Divider],
            layout[&Slot::Live(FOLDER, 0)] + room
        );
        assert_eq!(shell.live_item_of(30), Some(&items[1]));
    });
    // Its item gone from the lists, it's a tab like the folder's others, over the headings.
    let without = vec![items[0].clone(), items[2].clone()];
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Items(without), just_now(), cx)
    });
    draw(&shell, cx);
    shell.read_with(cx, |shell, _| {
        let layout = shell.layout();
        assert_eq!(
            layout[&Slot::Tab(30)],
            layout[&Slot::Tab(FOLDER)] + ROW_PITCH
        );
        assert_eq!(
            layout[&Slot::Live(FOLDER, 0)],
            layout[&Slot::Tab(30)] + ROW_PITCH
        );
        assert_eq!(shell.live_item_of(30), None);
    });
    // Closed, it has no rows, and its row ends with the review requests waiting.
    report(&shell, cx, false, &[30]);
    shell.read_with(cx, |shell, _| {
        assert!(!shell.layout().contains_key(&Slot::Live(FOLDER, 0)));
        assert_eq!(shell.live_waiting(FOLDER), 1);
    });
}

/// The folder holding tab 30 at #7, 31 at a page of its own and 32 at #3, in the order they were
/// opened, and its lists' answer.
fn three_tabs(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) {
    let items = vec![
        item(LiveList::ReviewRequests, 3),
        item(LiveList::Created, 7),
    ];
    let mut urls = host.pinned_urls.borrow_mut();
    urls.insert(30, items[1].url());
    urls.insert(31, "https://example.com/".into());
    urls.insert(32, items[0].url());
    drop(urls);
    report(shell, cx, true, &[30, 31, 32]);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Items(items), just_now(), cx)
    });
    draw(shell, cx);
}

#[gpui::test]
fn next_and_previous_tab_go_down_the_rows_as_drawn(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    three_tabs(&shell, cx, &host);
    shell.read_with(cx, |shell, _| {
        // The page's own tab, then #3's under Review requests, then #7's under Created by me.
        assert_eq!(shell.sidebar_rows(), [1, 31, 32, 30, 3]);
        let layout = shell.layout();
        let tops: Vec<f32> = [31, 32, 30]
            .iter()
            .map(|tab| layout[&Slot::Tab(*tab)])
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]));
    });
}

#[gpui::test]
fn a_tab_in_its_items_place_stays_in_the_folder_and_leaves_it_as_a_tab(cx: &mut TestAppContext) {
    use crate::shell::drag::detents::{X, drag_to, press};
    let (shell, cx, host) = window(cx);
    three_tabs(&shell, cx, &host);
    host.take_edits();
    // Dragged up among the folder's rows, it goes back to its item's place.
    let y = press(&shell, cx, 30);
    drag_to(&shell, cx, X, y - 2. * ROW_PITCH);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.moves.get(), 0);
    // Out of the folder, up by pinned tab 1, it's an ordinary pinned tab.
    let top = shell.read_with(cx, |shell, _| super::super::drag::detents::row_y(shell, 1));
    let y = press(&shell, cx, 30);
    drag_to(&shell, cx, X, y + 4.);
    drag_to(&shell, cx, X, top + ROW_PITCH / 2.);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.moves.get(), 1);
    assert_eq!(host.take_edits(), ["move 30 true 0 0"]);
}

#[gpui::test]
fn convert_to_folder_ends_its_lists_and_keeps_its_tabs(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    shell.update(cx, |shell, cx| {
        shell.tab_menu(FOLDER, point(px(10.), px(10.)), cx)
    });
    cx.update(|_, cx| host.choose("Convert to Folder", cx));
    assert_eq!(host.take_edits(), [format!("end live folder {FOLDER}")]);
    // Tabs in their items' places stay in the order they were drawn in.
    three_tabs(&shell, cx, &host);
    host.take_edits();
    shell.update(cx, |shell, cx| shell.convert_live_folder(FOLDER, cx));
    assert_eq!(
        host.take_edits(),
        [
            format!("move 32 true {FOLDER} 2"),
            format!("move 30 true {FOLDER} 2"),
            format!("end live folder {FOLDER}"),
        ]
    );
    // The core reports a plain folder: its lists are forgotten.
    shell.update(cx, |shell, cx| {
        let mut folders = Folders::default();
        folders
            .folders
            .insert(FOLDER, ("Pull Requests".into(), true));
        shell.set_tabs(1, vec![1, FOLDER], vec![3], Vec::new(), folders, cx);
        assert!(shell.live.is_empty());
        assert_eq!(shell.live_source(FOLDER), None);
    });
}

/// The core's report of Space 1 with only `folder` after pinned tab 1, open and empty: a Live
/// Folder if `live`, else a plain one, or with no folder at all if `folder` is 0.
fn report_only(shell: &Entity<Shell>, cx: &mut VisualTestContext, folder: u64, live: bool) {
    shell.update(cx, |shell, cx| {
        let mut folders = Folders::default();
        let mut pinned = vec![1];
        if folder != 0 {
            folders
                .folders
                .insert(folder, ("Pull Requests".into(), true));
            if live {
                folders.live.insert(folder, LiveSource::PullRequests);
            }
            pinned.push(folder);
        }
        shell.set_tabs(1, pinned, vec![3], Vec::new(), folders, cx);
    });
    draw(shell, cx);
}

/// The Live Folder ⌘Z made again, in the edits that made it and put it back after pinned tab 1.
fn made_live(host: &Recorder) -> u64 {
    let edits = host.take_edits();
    let made: u64 = (edits.first().and_then(|edit| edit.split(' ').nth(2)))
        .and_then(|id| id.parse().ok())
        .unwrap_or_else(|| panic!("no Live Folder made in {edits:?}"));
    assert_eq!(
        edits,
        [
            format!("live folder {made} 1 PullRequests Pull Requests"),
            format!("move folder {made} 0 1"),
        ]
    );
    made
}

#[gpui::test]
fn undo_brings_a_live_folder_deleted_or_unticked_back_live(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    host.take_edits();
    shell.update(cx, |shell, cx| {
        shell.tab_menu(FOLDER, point(px(10.), px(10.)), cx)
    });
    cx.update(|_, cx| host.choose("Delete Folder", cx));
    assert_eq!(host.take_edits(), [format!("delete folder {FOLDER}")]);
    report_only(&shell, cx, 0, false);
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    let made = made_live(&host);
    // Unticked in the Space's menu, it comes back live too.
    report_only(&shell, cx, made, true);
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(10.), px(10.)), cx)
    });
    cx.update(|_, cx| host.choose("GitHub Pull Requests", cx));
    assert_eq!(host.take_edits(), [format!("delete folder {made}")]);
    report_only(&shell, cx, 0, false);
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    made_live(&host);
}

#[gpui::test]
fn undo_makes_a_folder_converted_live_again_and_redo_converts_it_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    host.take_edits();
    shell.update(cx, |shell, cx| {
        shell.tab_menu(FOLDER, point(px(10.), px(10.)), cx)
    });
    cx.update(|_, cx| host.choose("Convert to Folder", cx));
    assert_eq!(host.take_edits(), [format!("end live folder {FOLDER}")]);
    report_only(&shell, cx, FOLDER, false);
    // The core makes a Live Folder new, so one is made in the plain one's place, and the plain
    // one, emptied into it, goes.
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    let edits = host.take_edits();
    let made: u64 = edits[0].split(' ').nth(2).unwrap().parse().unwrap();
    assert_eq!(
        edits,
        [
            format!("live folder {made} 1 PullRequests Pull Requests"),
            format!("move folder {made} 0 1"),
            format!("delete folder {FOLDER}"),
        ]
    );
    report_only(&shell, cx, made, true);
    assert!(shell.update(cx, |shell, cx| shell.redo(cx)));
    assert_eq!(host.take_edits(), [format!("end live folder {made}")]);
}

#[gpui::test]
fn an_answer_the_folder_holds_already_draws_nothing(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    report(&shell, cx, true, &[]);
    let answered = just_now();
    let items = || LiveAnswer::Items(vec![item(LiveList::Created, 7)]);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, items(), answered, cx)
    });
    draw(&shell, cx);
    let drawn = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = drawn.clone();
    let _watch = cx.update(|_, cx| cx.observe(&shell, move |_, _| counted.set(counted.get() + 1)));
    // The core's kept answer, for each ask inside its two minutes, is what the folder holds.
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, items(), answered, cx);
        shell.set_live_answer(FOLDER, items(), answered, cx);
    });
    assert_eq!(drawn.get(), 0);
    // The same rows the site gave again later make the folder's age new, and a failure shows.
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, items(), answered + 1, cx)
    });
    let again = drawn.get();
    assert!(again > 0);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Failed, answered + 2, cx)
    });
    let failed = drawn.get();
    assert!(failed > again);
    // A failure again, the core's kept one or the site's, changes nothing either.
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Failed, answered + 3, cx)
    });
    assert_eq!(drawn.get(), failed);
}

#[gpui::test]
fn another_profile_for_the_space_drops_the_old_accounts_rows_and_asks(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    report(&shell, cx, true, &[]);
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(
            FOLDER,
            LiveAnswer::Items(vec![item(LiveList::Created, 7)]),
            just_now(),
            cx,
        )
    });
    host.take();
    let rows =
        |shell: &Shell| (shell.live.get(&FOLDER)).map_or(0, |kept| kept.rows(|_| None).len());
    assert!(shell.read_with(cx, |shell, _| rows(shell)) > 0);
    // Set Profile: the core reports Space 1 in profile 2.
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 2, "".into(), "".into(), "".into());
        let other = (2, "Other".into(), None, 1, "".into(), "".into(), "".into());
        let agent = (
            9,
            "Agent".into(),
            None,
            1,
            "".into(),
            "claude".into(),
            "".into(),
        );
        shell.set_spaces(vec![space, other, agent], cx);
    });
    assert_eq!(shell.read_with(cx, |shell, _| rows(shell)), 0);
    assert_eq!(
        fetches(&host),
        [format!(
            "fetch live folder {FOLDER} PullRequests refresh false"
        )]
    );
    // The new account's fetch failing keeps none of the old one's rows.
    shell.update(cx, |shell, cx| {
        shell.set_live_answer(FOLDER, LiveAnswer::Failed, just_now(), cx)
    });
    assert_eq!(shell.read_with(cx, |shell, _| rows(shell)), 0);
}

#[gpui::test]
fn a_live_folder_made_in_a_space_with_no_tabs_asks(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    draw(&shell, cx);
    host.take();
    shell.update(cx, |shell, cx| {
        let mut folders = Folders::default();
        folders
            .folders
            .insert(FOLDER, ("Pull Requests".into(), true));
        folders.live.insert(FOLDER, LiveSource::PullRequests);
        shell.set_tabs(1, vec![FOLDER], Vec::new(), Vec::new(), folders, cx);
    });
    draw(&shell, cx);
    assert_eq!(
        fetches(&host),
        [format!(
            "fetch live folder {FOLDER} PullRequests refresh false"
        )]
    );
}

#[gpui::test]
fn a_live_folder_brought_back_with_its_space_doesnt_ask_as_made(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    // Space 2's first report, as at launch, has the folder already.
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        let other = (2, "Other".into(), None, 1, "".into(), "".into(), "".into());
        let more = (4, "More".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space, other, more], cx);
        let mut folders = Folders::default();
        folders.folders.insert(40, ("Pull Requests".into(), false));
        folders.live.insert(40, LiveSource::PullRequests);
        shell.set_tabs(4, vec![40], Vec::new(), Vec::new(), folders, cx);
    });
    draw(&shell, cx);
    assert!(fetches(&host).is_empty());
}
