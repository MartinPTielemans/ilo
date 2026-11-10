//! History (#2337): the closed tabs and the profile's visited pages in one list, newest first, a
//! page once a day and a closed tab's page as the closed tab; asked for a page at a time once the
//! Library has drawn, found by the browser's search, filtered, marked where an agent's tab visited,
//! and removed a day at a time. Never in a private window.

use std::rc::Rc;

use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::{
    ARCHIVE_TOP, ARCHIVED_LEFT, ARCHIVED_PITCH, Archived, COLUMN_WIDTH, FILTER_WIDTH, FILTER_X,
    HEADING_BOTTOM, HEADING_HEIGHT, HEADING_TOP, Item, SEARCH_HEIGHT, SEARCH_TOP, Visited,
};
use crate::agent_history::{self, Past};
use crate::shell::{Command, Folders, Section, Shell, recorder::Recorder};

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
