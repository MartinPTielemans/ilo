//! History (#2337): the closed tabs and the profile's visited pages in one list, newest first, a
//! page once a day and a closed tab's page as the closed tab; asked for a page at a time once the
//! Library has drawn, found by the browser's search, filtered, marked where an agent's tab visited,
//! and removed a day at a time. Never in a private window.

use std::rc::Rc;

use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::history_list::history_text_end;
use super::{
    ARCHIVE_TOP, ARCHIVED_LEFT, ARCHIVED_PITCH, ARCHIVED_RIGHT, Archived, COLUMN_WIDTH,
    FILTER_WIDTH, FILTER_X, HEADING_BOTTOM, HEADING_HEIGHT, HEADING_TOP, Item, LIBRARY_WIDTH,
    Pointed, SEARCH_HEIGHT, SEARCH_TOP, Visited, history_when,
};
use crate::agent_history::{self, Past};
use crate::shell::{Command, Folders, Section, Shell, recorder::Recorder};
use crate::tokens::shell::{CLOSE_RIGHT, CLOSE_SIZE};

/// The profile of the window's Space.
const PROFILE: u64 = 7;

/// `hour` o'clock on 2026-11-`day` where the test runs, in milliseconds since the Unix epoch.
fn at(day: u32, hour: u32) -> f64 {
    let time = NaiveDate::from_ymd_opt(2026, 11, day)
        .and_then(|day| day.and_hms_opt(hour, 0, 0))
        .unwrap();
    Local
        .from_local_datetime(&time)
        .earliest()
        .unwrap()
        .timestamp_millis() as f64
}

fn url(name: &str) -> String {
    format!("https://{name}.example/")
}

fn page(name: &str, visited: f64) -> Visited {
    Visited {
        url: url(name).into(),
        title: name.to_string().into(),
        visited,
    }
}

fn closed(id: u64, name: &str, archived: f64, little: bool) -> Archived {
    Archived {
        id,
        url: url(name).into(),
        title: name.to_string().into(),
        archived,
        little,
    }
}

fn space(id: u64, profile: u64) -> crate::shell::SpaceEntry {
    (
        id,
        format!("Space {id}").into(),
        None,
        profile,
        "".into(),
        "".into(),
        "".into(),
    )
}

/// A window on Space 1, of profile 7, with Space 2 of profile 8 beside it, and the Library open on
/// History at rest, holding `archive`, its first frame drawn but not the one after.
fn history(
    archive: Vec<Archived>,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![space(1, PROFILE), space(2, 8)], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.set_archive(archive, cx);
        shell.section = Section::Archive;
        shell.run(Command::ViewLibrary, "test", cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    (shell, host, cx)
}

/// The frame after the Library's first.
fn next_frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.run_until_parked();
}

/// What History asked the browser for since the last time, each as its request and the rest:
/// profile, before, text and how many.
fn asked(host: &Recorder) -> Vec<(u64, String)> {
    (host.take().into_iter())
        .filter_map(|call| {
            let rest = call.strip_prefix("list history ")?.to_string();
            let mut words = rest.splitn(3, ' ');
            let (profile, request, rest) = (words.next()?, words.next()?, words.next()?);
            Some((request.parse().ok()?, format!("{profile} {rest}")))
        })
        .collect()
}

/// The one request History asked for, with what it asked.
fn one_ask(host: &Recorder) -> (u64, String) {
    let asked = asked(host);
    assert_eq!(asked.len(), 1, "asked {asked:?}");
    asked[0].clone()
}

fn answer(
    shell: &Entity<Shell>,
    request: u64,
    pages: Vec<Visited>,
    more: bool,
    cx: &mut VisualTestContext,
) {
    shell.update(cx, |shell, cx| {
        shell.history_listed(PROFILE, request, pages, more, cx)
    });
    cx.run_until_parked();
}

/// The list's lines: headings, "closed <title>", "visited <title>" and "visited <title> by
/// <agent>".
fn items(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let library = shell.library.as_ref().unwrap();
        let named = library.items.iter().map(|item| match item {
            Item::Heading(heading) => heading.to_string(),
            Item::Tab(index) => format!("closed {}", shell.archive[*index].title),
            Item::Visit(index) => {
                let visit = &library.history.pages[*index];
                match &visit.by {
                    Some(agent) => format!("visited {} by {agent}", visit.page.title),
                    None => format!("visited {}", visit.page.title),
                }
            }
            _ => "other".to_string(),
        });
        named.collect()
    })
}

/// The middle of the list's `row`th row, under the first heading.
fn row(row: usize) -> gpui::Point<gpui::Pixels> {
    let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM;
    point(
        px(COLUMN_WIDTH + ARCHIVED_LEFT + 60.),
        px(top + ARCHIVED_PITCH * (row as f32 + 0.5)),
    )
}

fn filter(host: &Recorder, choice: &str, cx: &mut VisualTestContext) {
    let filter = point(
        px(FILTER_X + FILTER_WIDTH / 2.),
        px(SEARCH_TOP + SEARCH_HEIGHT / 2.),
    );
    cx.simulate_click(filter, Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| host.choose(choice, cx));
    cx.run_until_parked();
}

fn search(shell: &Entity<Shell>, text: &str, cx: &mut VisualTestContext) {
    let field = shell.read_with(cx, |shell, _| {
        shell.library.as_ref().unwrap().search.clone()
    });
    let text = text.to_string();
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.set_text(text.clone().into(), text.len(), window, cx)
        })
    });
    cx.run_until_parked();
}

fn empty_line(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> &'static str {
    shell.read_with(cx, |shell, _| {
        let unsearched = shell.library.as_ref().unwrap().query.is_empty();
        shell.empty_line(Section::Archive, unsearched)
    })
}

/// The Library asks for the first page of history only once it has drawn, never as it opens, and
/// once; then for the older pages as its list nears its end, from before the oldest it has, one at
/// a time, until there are none.
#[gpui::test]
fn history_asks_a_page_at_a_time_after_the_library_opens(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    assert!(asked(&host).is_empty(), "asked as the Library opened");
    next_frame(cx);
    let (first, what) = one_ask(&host);
    assert_eq!(what, "7 0 \"\" 60");
    next_frame(cx);
    assert!(asked(&host).is_empty(), "asked again while asking");

    // Five rows are all near the end, so the older pages come next.
    let pages: Vec<_> = (0..5)
        .map(|n| page(&format!("p{n}"), at(9, 20 - n)))
        .collect();
    answer(&shell, first, pages, true, cx);
    let (older, what) = one_ask(&host);
    let before = (at(9, 16) * 1000.) as i64;
    assert_eq!(what, format!("7 {before} \"\" 60"));
    next_frame(cx);
    assert!(asked(&host).is_empty(), "asked again while asking");

    let lines = items(&shell, cx).len();
    answer(&shell, older, vec![page("p5", at(9, 15))], false, cx);
    assert_eq!(
        items(&shell, cx).len(),
        lines + 1,
        "the older page wasn't added"
    );
    let count = shell.read_with(cx, |shell, _| {
        shell.library.as_ref().unwrap().list.item_count()
    });
    assert_eq!(count, lines + 1, "the list didn't grow at its end");
    next_frame(cx);
    assert!(
        asked(&host).is_empty(),
        "asked for more when there were none"
    );
}

/// The closed tabs and the visited pages list together, newest first, a page that's also a closed
/// tab once, as the closed tab. While older pages are left, a closed tab older than the oldest page
/// waits for them, so each page only adds to the list's end.
#[gpui::test]
fn history_lists_closed_tabs_and_visited_pages_together(cx: &mut TestAppContext) {
    let archive = vec![
        closed(1, "shop", at(9, 15), false),
        closed(2, "news", at(9, 9), false),
    ];
    let (shell, host, cx) = history(archive, cx);
    assert_eq!(items(&shell, cx), ["Today", "closed shop", "closed news"]);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let pages = vec![
        page("mail", at(9, 14)),
        page("shop", at(9, 13)),
        page("maps", at(9, 10)),
    ];
    answer(&shell, first, pages, true, cx);
    assert_eq!(
        items(&shell, cx),
        ["Today", "closed shop", "visited mail", "visited maps"]
    );
    let (older, _) = one_ask(&host);
    answer(&shell, older, vec![page("docs", at(9, 8))], false, cx);
    assert_eq!(
        items(&shell, cx),
        [
            "Today",
            "closed shop",
            "visited mail",
            "visited maps",
            "closed news",
            "visited docs"
        ]
    );
}

/// A page visited more than once a day is one row that day, at its last visit, though its visits
/// that day come in two pages; another day's visit is a row of its own.
#[gpui::test]
fn history_lists_a_page_once_a_day(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(
        &shell,
        first,
        vec![page("mail", at(9, 14)), page("maps", at(9, 12))],
        true,
        cx,
    );
    let (older, _) = one_ask(&host);
    answer(
        &shell,
        older,
        vec![page("mail", at(9, 11)), page("mail", at(8, 20))],
        false,
        cx,
    );
    assert_eq!(
        items(&shell, cx),
        ["Today", "visited mail", "visited maps", "visited mail"]
    );
    let times = shell.read_with(cx, |shell, _| {
        let pages = &shell.library.as_ref().unwrap().history.pages;
        pages
            .iter()
            .map(|visit| visit.page.visited)
            .collect::<Vec<_>>()
    });
    assert_eq!(times, [at(9, 14), at(9, 12), at(8, 20)]);
}

/// The Filter shows once History has two kinds of line, and its menu lists the closed tabs alone,
/// the visited pages alone, or all, each saying what fills it while it's empty, and with a search,
/// that nothing matches.
#[gpui::test]
fn each_filter_lists_its_lines_and_says_when_it_has_none(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    assert_eq!(
        empty_line(&shell, cx),
        "Pages you visit and tabs you close show here."
    );
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, vec![page("mail", at(9, 14))], false, cx);
    assert!(
        !shell.read_with(cx, |shell, _| shell.archive_filtered()),
        "a Filter with one kind of line"
    );
    shell.update(cx, |shell, cx| {
        shell.set_archive(vec![closed(1, "shop", at(9, 15), false)], cx)
    });
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.archive_filtered()),
        "no Filter with closed tabs and visited pages"
    );

    filter(&host, "Visited", cx);
    assert_eq!(items(&shell, cx), ["Today", "visited mail"]);
    filter(&host, "Closed Tabs", cx);
    assert_eq!(items(&shell, cx), ["Today", "closed shop"]);
    filter(&host, "All", cx);
    assert_eq!(items(&shell, cx), ["Today", "closed shop", "visited mail"]);

    search(&shell, "zzz", cx);
    let (found, what) = one_ask(&host);
    assert_eq!(what, "7 0 \"zzz\" 60");
    answer(&shell, found, Vec::new(), false, cx);
    assert!(items(&shell, cx).is_empty());
    assert_eq!(empty_line(&shell, cx), "No pages match.");
    filter(&host, "Visited", cx);
    assert_eq!(empty_line(&shell, cx), "No visited pages match.");
    filter(&host, "Closed Tabs", cx);
    assert_eq!(empty_line(&shell, cx), "No closed tabs match.");

    // The profile has had pages, so the Filter stays though none are left.
    search(&shell, "", cx);
    let (all, _) = one_ask(&host);
    answer(&shell, all, Vec::new(), false, cx);
    filter(&host, "Visited", cx);
    assert_eq!(empty_line(&shell, cx), "Pages you visit show here.");
    shell.update(cx, |shell, cx| {
        shell.set_archive(Vec::new(), cx);
        shell.set_archived_tasks(vec![task()], cx);
    });
    cx.run_until_parked();
    filter(&host, "Closed Tabs", cx);
    assert_eq!(
        empty_line(&shell, cx),
        "Closed Today tabs stay here for 30 days."
    );
}

fn task() -> crate::agents::task::ArchivedTask {
    crate::agents::task::ArchivedTask {
        agent: "task-1".into(),
        title: "Lisbon museum hours".into(),
        asked: "Which museums in Lisbon open on Monday?".into(),
        dot: crate::agents::task::Dot::Done,
        about: "Claude · Done 13:31 · 1 page".into(),
        left: at(9, 13),
    }
}

/// A search asks the browser for the pages with its words. Until it answers, the pages listed are
/// searched as typed; its answer is the list, though a page's title doesn't hold the text as typed.
#[gpui::test]
fn a_search_asks_the_history(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(vec![closed(1, "wiki-shop", at(9, 15), false)], cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let pages = vec![page("wiki-mail", at(9, 14)), page("maps", at(9, 12))];
    answer(&shell, first, pages, false, cx);

    search(&shell, "Wiki", cx);
    let (found, what) = one_ask(&host);
    assert_eq!(what, "7 0 \"wiki\" 60");
    assert_eq!(
        items(&shell, cx),
        ["Today", "closed wiki-shop", "visited wiki-mail"]
    );
    let pages = vec![
        page("wiki-mail", at(9, 14)),
        page("encyclopedia", at(9, 11)),
    ];
    answer(&shell, found, pages, false, cx);
    assert_eq!(
        items(&shell, cx),
        [
            "Today",
            "closed wiki-shop",
            "visited wiki-mail",
            "visited encyclopedia"
        ]
    );
    // An answer for a search since replaced is left.
    search(&shell, "wik", cx);
    let (_, what) = one_ask(&host);
    assert_eq!(what, "7 0 \"wik\" 60");
    answer(&shell, found, vec![page("old", at(9, 10))], false, cx);
    assert!(!items(&shell, cx).contains(&"visited old".to_string()));
}

fn past(agent: &str, started: u64, ended: u64, pages: &[&str]) -> Past {
    Past {
        id: format!("{agent}-{started}"),
        title: "A task".into(),
        agent: agent.into(),
        started,
        ended,
        pages: (pages.iter())
            .map(|name| (name.to_string(), url(name)))
            .collect(),
        orders: 0,
        messages: 0,
        steps: Vec::new(),
        answers: String::new(),
    }
}

/// A page an agent's task's tab visited is marked with its agent: one the task recorded, last
/// visited that day while it ran. Another page then, the page after the task, or a task that ended
/// 30 days ago or more, which Library › Agents no longer keeps, leaves it unmarked.
#[gpui::test]
fn a_page_an_agents_tab_visited_is_marked(cx: &mut TestAppContext) {
    let now = crate::usage::now();
    let day = 24 * 60 * 60;
    let pasts = [
        past("Claude", now - 600, now - 300, &["shop"]),
        past("Codex", now - 29 * day - 600, now - 29 * day, &["maps"]),
        past(
            "Claude Code",
            now - 31 * day - 600,
            now - 31 * day,
            &["mail"],
        ),
    ];
    let (shell, host, cx) = history(Vec::new(), cx);
    host.agent_history
        .borrow_mut()
        .insert(PROFILE, agent_history::write(&pasts));
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let ms = |seconds: u64| seconds as f64 * 1000.;
    let pages = vec![
        page("shop", ms(now - 400)),
        page("news", ms(now - 450)),
        page("maps", ms(now - 29 * day - 300)),
        page("mail", ms(now - 31 * day - 300)),
    ];
    answer(&shell, first, pages, false, cx);
    let marked: Vec<String> = (items(&shell, cx).into_iter())
        .filter(|item| item.starts_with("visited"))
        .collect();
    assert_eq!(
        marked,
        [
            "visited shop by Claude",
            "visited news",
            "visited maps by Codex",
            "visited mail"
        ]
    );
    // After its task ended, the page is the user's visit.
    let (shell, host, cx) = history(Vec::new(), cx);
    host.agent_history
        .borrow_mut()
        .insert(PROFILE, agent_history::write(&pasts[..1]));
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, vec![page("shop", ms(now - 100))], false, cx);
    assert_eq!(items(&shell, cx), ["Today", "visited shop"]);
}

/// Remove from History in a visited page's menu deletes that page's visits on its row's day, from
/// its midnight to the next, and its row goes; another day's row stays. Deleting history asks for
/// what's left.
#[gpui::test]
fn remove_from_history_takes_the_rows_day(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let pages = vec![page("mail", at(9, 14)), page("mail", at(8, 20))];
    answer(&shell, first, pages, false, cx);
    cx.simulate_mouse_down(row(0), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    let titles: Vec<String> = menu.iter().map(|(title, _)| title.to_string()).collect();
    assert_eq!(
        titles,
        ["Open in New Tab", "Copy URL", "", "Remove from History"]
    );
    cx.update(|_, cx| host.choose("Remove from History", cx));
    cx.run_until_parked();
    let micros = |ms: f64| (ms * 1000.) as i64;
    assert_eq!(
        host.take(),
        [format!(
            "remove history 7 {} {} {}",
            url("mail"),
            micros(at(9, 0)),
            micros(at(10, 0))
        )]
    );
    let times = shell.read_with(cx, |shell, _| {
        let pages = &shell.library.as_ref().unwrap().history.pages;
        pages
            .iter()
            .map(|visit| visit.page.visited)
            .collect::<Vec<_>>()
    });
    assert_eq!(times, [at(8, 20)]);

    // The core says it's gone, and History asks for what's left, as much as it had.
    shell.update(cx, |shell, cx| shell.history_forgotten(8, cx));
    assert!(asked(&host).is_empty(), "another profile's deletion");
    shell.update(cx, |shell, cx| shell.history_forgotten(PROFILE, cx));
    let (_, what) = one_ask(&host);
    assert_eq!(what, "7 0 \"\" 60");
}

fn library_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.library_open)
}

/// The calls History made since the last time, past its asks for pages and favicons.
fn made(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls
        .filter(|call| !call.starts_with("list history ") && !call.starts_with("page favicon"))
        .collect()
}

/// A click on a visited page opens it in Peek over the tab shown, growing from the row, as a
/// closed tab's does, and the Library stays; kept, it's the window's tab and the Library closes.
/// ⌘-click opens it in a new tab at once and closes the Library (the orchestrator, 11-14).
#[gpui::test]
fn a_click_on_a_visited_page_opens_it_in_peek(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, vec![page("mail", at(9, 14))], false, cx);
    made(&host);
    cx.simulate_click(row(0), Modifiers::none());
    let (x, y) = (f32::from(row(0).x), f32::from(row(0).y));
    let peek = format!("open preview over 10 {} from {x},{y}", url("mail"));
    assert_eq!(made(&host), [peek]);
    assert!(library_open(&shell, cx));
    shell.update(cx, |shell, cx| shell.kept_from_preview(cx));
    cx.run_until_parked();
    assert!(!library_open(&shell, cx));

    shell.update(cx, |shell, cx| shell.run(Command::ViewLibrary, "test", cx));
    shell.update(cx, |shell, _| shell.library_motion = None);
    next_frame(cx);
    if let Some((again, _)) = asked(&host).pop() {
        answer(&shell, again, vec![page("mail", at(9, 14))], false, cx);
    }
    made(&host);
    let command = cx.update(|_, cx| Modifiers::secondary_key(cx));
    cx.simulate_click(row(0), command);
    assert_eq!(made(&host), [format!("open {} false 0 0", url("mail"))]);
    assert!(!library_open(&shell, cx));
}

/// Where a History row's trash is, at the end of the list's `row`th row, as the sidebar's ✕ is
/// at the end of a tab's.
fn trash(
    shell: &Entity<Shell>,
    row: usize,
    cx: &mut VisualTestContext,
) -> gpui::Point<gpui::Pixels> {
    let end = shell.read_with(cx, |shell, _| {
        shell.library_x() + LIBRARY_WIDTH - ARCHIVED_RIGHT
    });
    point(px(end - CLOSE_RIGHT - CLOSE_SIZE / 2.), self::row(row).y)
}

fn pointed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Pointed> {
    shell.read_with(cx, |shell, _| {
        shell.library.as_ref().unwrap().history.pointed
    })
}

/// A History row's trash shows only under the pointer, as a tab row's ✕ does, and goes as the
/// pointer leaves: at rest the row's text has its whole width. A closed tab's trash deletes it
/// from the archive, which ⌘Z brings back, and the Library stays (the orchestrator, 11-14).
#[gpui::test]
fn a_closed_tabs_trash_shows_under_the_pointer(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(vec![closed(5, "news", at(9, 15), false)], cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, Vec::new(), false, cx);
    let away = point(px(COLUMN_WIDTH + 4.), px(4.));
    cx.simulate_mouse_move(away, None, Modifiers::none());
    next_frame(cx);
    assert_eq!(pointed(&shell, cx), None);
    cx.simulate_mouse_move(row(0), None, Modifiers::none());
    next_frame(cx);
    assert_eq!(pointed(&shell, cx), Some(Pointed::Closed(5)));
    cx.simulate_mouse_move(away, None, Modifiers::none());
    next_frame(cx);
    assert_eq!(
        pointed(&shell, cx),
        None,
        "the trash after the pointer left"
    );

    let at = trash(&shell, 0, cx);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    next_frame(cx);
    made(&host);
    host.take_edits();
    cx.simulate_click(at, Modifiers::none());
    let calls = made(&host);
    assert!(
        !calls.iter().any(|call| call.contains("preview")),
        "{calls:?}"
    );
    assert_eq!(host.take_edits(), ["archive remove 5 as 9001"]);
    assert!(library_open(&shell, cx));
}

/// A long title has the row's whole width at rest, running over the trash's place, and truncates 4
/// before the trash only while the trash shows, as a tab row's title does before its ✕ (the
/// orchestrator's call 2 on #2337, 11-14).
#[test]
fn a_title_has_the_trashs_room_until_the_trash_shows() {
    let trash_start = CLOSE_RIGHT + CLOSE_SIZE;
    let at_rest = history_text_end(false);
    assert!(
        at_rest < trash_start,
        "at rest a title ends {at_rest} from the row's end, short of the trash's place, which starts {trash_start} from it"
    );
    assert_eq!(
        history_text_end(true),
        trash_start + 4.,
        "while the trash shows, a title ends 4 before it"
    );
}

/// A visited page's trash removes its visits that day, and its row goes at once.
#[gpui::test]
fn a_visited_pages_trash_removes_its_day(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, vec![page("mail", at(9, 14))], false, cx);
    let spot = trash(&shell, 0, cx);
    cx.simulate_mouse_move(spot, None, Modifiers::none());
    next_frame(cx);
    assert_eq!(pointed(&shell, cx), Some(Pointed::Visited(0)));
    made(&host);
    cx.simulate_click(spot, Modifiers::none());
    let micros = |ms: f64| (ms * 1000.) as i64;
    let removed = format!(
        "remove history 7 {} {} {}",
        url("mail"),
        micros(at(9, 0)),
        micros(at(10, 0))
    );
    assert_eq!(made(&host), [removed]);
    assert!(items(&shell, cx).iter().all(|item| item != "visited mail"));
}

/// History is the profile's of the Space shown: another profile's Space lists its own, from the
/// top, and the first's answer is left.
#[gpui::test]
fn history_is_the_shown_spaces_profiles(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(Vec::new(), cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.run_until_parked();
    let (_, what) = one_ask(&host);
    assert_eq!(what, "8 0 \"\" 60");
    answer(&shell, first, vec![page("mail", at(9, 14))], false, cx);
    assert!(!items(&shell, cx).contains(&"visited mail".to_string()));
}

/// A private window has no History: its Library offers none, asks the browser for no pages, and
/// lists none it's told of.
#[gpui::test]
fn a_private_window_lists_no_history(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        Shell::private_window(browser, 9, "Incognito".into(), PROFILE, cx)
    });
    shell.update(cx, |shell, cx| {
        shell.run(Command::ViewArchive, "test", cx);
        shell.run(Command::ViewLibrary, "test", cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    next_frame(cx);
    next_frame(cx);
    assert!(
        asked(&host).is_empty(),
        "a private window asked for history"
    );
    shell.update(cx, |shell, cx| {
        assert!(!shell.has_section(Section::Archive));
        shell.history_listed(PROFILE, 1, vec![page("mail", at(9, 14))], false, cx);
        let library = shell.library.as_ref().unwrap();
        assert!(library.history.pages.is_empty());
        assert!(
            !library
                .items
                .iter()
                .any(|item| matches!(item, Item::Visit(_)))
        );
    });
}

/// A row's time is on the system's clock, 12- or 24-hour, today, this week and last; a date has no
/// time to show.
#[test]
fn row_times_follow_the_systems_clock() {
    // Today the 13th, yesterday, this week from the 9th, last week from the 2nd.
    let starts = [at(13, 0), at(12, 0), at(9, 0), at(2, 0)];
    let minute = 5. * 60. * 1000.;
    let today = at(13, 14) + minute;
    assert_eq!(history_when(today, starts, false), "14:05");
    assert_eq!(history_when(today, starts, true), "2:05 PM");
    let morning = at(12, 9) + minute;
    assert_eq!(history_when(morning, starts, true), "9:05 AM");
    assert_eq!(history_when(morning, starts, false), "09:05");
    let week = at(10, 21) + minute;
    assert_eq!(history_when(week, starts, true), "Tue 9:05 PM");
    assert_eq!(history_when(week, starts, false), "Tue 21:05");
    assert_eq!(history_when(at(1, 14), starts, true), "1 Nov");
}
