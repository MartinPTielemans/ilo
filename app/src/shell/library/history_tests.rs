//! History (#2337): the closed tabs and the profile's visited pages in one list, newest first, a
//! page once a day and a closed tab's page as the closed tab; asked for a page at a time once the
//! Library has drawn, found by the browser's search, filtered, marked where an agent's tab visited,
//! and removed a day at a time. Never in a private window.

use std::rc::Rc;

use chrono::{Local, NaiveDate, TimeZone};
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::history_list::history_text_end;
use super::{
    ARCHIVE_TOP, ARCHIVE_WIDTH, ARCHIVED_LEFT, ARCHIVED_PITCH, ARCHIVED_RIGHT, Archived,
    COLUMN_WIDTH, FILTER_CHEVRON, FILTER_PAD_RIGHT, HEADING_BOTTOM, HEADING_HEIGHT, HEADING_TOP,
    Item, LIBRARY_WIDTH, Pointed, SEARCH_HEIGHT, SEARCH_RIGHT, SEARCH_TOP, Visited, history_when,
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
            Item::Day(day) => super::day_heading(*day),
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
        px(COLUMN_WIDTH + ARCHIVE_WIDTH - SEARCH_RIGHT - FILTER_PAD_RIGHT - FILTER_CHEVRON / 2.),
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
        about: "Claude · Done".into(),
        ended: at(9, 13),
        pages: "1 page".into(),
    }
}

/// After a quit and a launch, the composer's Latest tasks lists the task that run left (#4105), in
/// the Spaces of its own profile and no other's, with no card at the sidebar's foot. A click on
/// it opens its conversation, which has ilo's one line and the reply field, and from then it's a
/// card at the foot as any task opened in this run. The click is made by the card's own call:
/// the composer keeps no record of where a card is drawn.
#[cfg(unix)]
#[gpui::test]
fn latest_tasks_lists_a_task_a_run_before_left(cx: &mut TestAppContext) {
    use crate::agents::task::{Line, TaskAction};
    use crate::agents::tests::kept::past::after_a_quit;

    let (agents, past) = after_a_quit("Lisbon seats", cx);
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    // No tab is shown in either Space, so each shows the composer.
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![space(1, PROFILE), space(2, 8)], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    let give = |cx: &mut VisualTestContext| {
        let (status, cards, archived, kept) = agents.read_with(cx, |agents, cx| {
            (
                agents.status(),
                agents.shown_cards(cx),
                agents.archived_tasks(cx),
                agents.kept_cards(cx),
            )
        });
        let conversation = |agent: &str, cx: &gpui::App| agents.read(cx).conversation(agent, cx);
        let shells = [shell.downgrade()];
        cx.update(|_, cx| {
            crate::shell::send_agents(status, cards, archived, kept, conversation, &shells, cx)
        });
        cx.run_until_parked();
    };
    // What the composer lists, by the tasks' ids.
    let listed = |cx: &mut VisualTestContext| {
        let cards = shell.read_with(cx, |shell, _| shell.latest_tasks(3));
        (cards.into_iter().map(|card| card.agent)).collect::<Vec<_>>()
    };
    // The composer draws without it, and its card comes in as the agents send it.
    assert!(listed(cx).is_empty());
    give(cx);
    assert_eq!(listed(cx), [past.as_str()]);
    assert!(shell.read_with(cx, |shell, _| shell.task_cards.is_empty()));
    // Another profile's Space lists none of it.
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.run_until_parked();
    assert!(listed(cx).is_empty());
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    cx.run_until_parked();
    assert_eq!(listed(cx), [past.as_str()]);

    // The card's click.
    made(&host);
    let agent: gpui::SharedString = past.clone().into();
    shell.update(cx, |shell, cx| shell.open_latest(agent, cx));
    let asked = made(&host);
    assert!(asked.contains(&format!("task {past} Restore")), "{asked:?}");
    agents.update(cx, |agents, cx| {
        _ = agents.task_action(&past, TaskAction::Restore, cx)
    });
    give(cx);
    let shown = shell.read_with(cx, |shell, _| shell.talk_agent());
    assert_eq!(shown.as_deref(), Some(past.as_str()));
    // The window asked for its conversation, which the agents send as they send the rest.
    give(cx);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, cx| shell.reply_drawn(cx).is_some()));
    let talk = agents.read_with(cx, |agents, cx| agents.conversation(&past, cx).unwrap());
    assert!(
        matches!(&talk.lines[..], [Line::Ilo(_)]),
        "{:?}",
        talk.lines
    );
    shell.read_with(cx, |shell, _| {
        assert!(shell.kept_cards.is_empty());
        assert_eq!(shell.task_cards.len(), 1);
    });
}

/// A task that ended in a run before is there to reply to after a quit (#4105), on the agents as
/// they are after that quit and the launch (`after_a_quit`), the test doing the app's part between
/// them and the window. Its row is in History; a click on it brings its card back to the Space of
/// its own profile and no other, with its conversation open and the reply field in it, though it
/// has no Space of its own. A reply is its task's, and once its run opens a page the area is the
/// Space made for it, and goes with that Space.
#[cfg(unix)]
#[gpui::test]
fn a_click_on_a_past_tasks_row_opens_its_conversation(cx: &mut TestAppContext) {
    use crate::agents::task::TaskAction;
    use crate::agents::tests::kept::past::{after_a_quit, opens_a_page};

    let (agents, past) = after_a_quit("Lisbon seats", cx);
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, host, cx) = history(Vec::new(), cx);
    // What the app sends its windows as the agents change, by the way it sends it.
    let give = |cx: &mut VisualTestContext| {
        let (status, cards, archived, kept) = agents.read_with(cx, |agents, cx| {
            (
                agents.status(),
                agents.shown_cards(cx),
                agents.archived_tasks(cx),
                agents.kept_cards(cx),
            )
        });
        assert!(
            cards.iter().all(|card| card.home == 1),
            "a card outside its profile's Space"
        );
        let conversation = |agent: &str, cx: &gpui::App| agents.read(cx).conversation(agent, cx);
        let shells = [shell.downgrade()];
        cx.update(|_, cx| {
            crate::shell::send_agents(status, cards, archived, kept, conversation, &shells, cx)
        });
        cx.run_until_parked();
    };
    give(cx);
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, Vec::new(), false, cx);
    assert_eq!(items(&shell, cx).len(), 2, "today's heading and the task");
    let shown = |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.talk_agent());
    assert_eq!(shown(cx), None);

    made(&host);
    cx.simulate_click(row(0), Modifiers::none());
    let asked = made(&host);
    assert!(asked.contains(&format!("task {past} Restore")), "{asked:?}");
    assert!(!library_open(&shell, cx));
    let (_, back) = agents.update(cx, |agents, cx| {
        agents.task_action(&past, TaskAction::Restore, cx)
    });
    assert_eq!(back.as_deref(), Some(past.as_str()));
    let own = agents.read_with(cx, |agents, _| agents.task(&past).unwrap().space);
    assert_eq!(own, 0);
    give(cx);
    assert_eq!(shown(cx).as_deref(), Some(past.as_str()));
    assert!(made(&host).contains(&format!("show task {past}")));
    // The window asked for its conversation, which the agents send as they send the rest.
    give(cx);
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let reply = shell.read_with(cx, |shell, cx| shell.reply_drawn(cx));
    assert!(reply.is_some(), "no reply field");

    cx.simulate_input("And for Sunday?");
    cx.simulate_keystrokes("enter");
    let said = format!(r#"task {past} Reply("And for Sunday?")"#);
    assert!(made(&host).contains(&said));
    let reply = TaskAction::Reply("And for Sunday?".into());
    agents.update(cx, |agents, cx| _ = agents.task_action(&past, reply, cx));
    let own = opens_a_page(&agents, &past, cx);
    assert_ne!(own, 0);
    // The cards come before the core tells of the Space, and the conversation stays where it is.
    give(cx);
    assert_eq!(shown(cx).as_deref(), Some(past.as_str()));
    let spaces = |own: Option<u64>, cx: &mut VisualTestContext| {
        let mut spaces = vec![space(1, PROFILE), space(2, 8)];
        if let Some(own) = own {
            host.space_agents
                .borrow_mut()
                .insert(own, past.clone().into());
            let mut entry = space(own, PROFILE);
            entry.5 = "Claude".into();
            spaces.push(entry);
        } else {
            host.space_agents.borrow_mut().clear();
        }
        shell.update(cx, |shell, cx| shell.set_spaces(spaces, cx));
        cx.run_until_parked();
    };
    spaces(Some(own), cx);
    assert_eq!(shown(cx).as_deref(), Some(past.as_str()));
    assert_eq!(shell.read_with(cx, |shell, _| shell.task_open()), Some(own));
    // The Space gone, as when the task goes for good, its area goes with it.
    spaces(None, cx);
    assert_eq!(shown(cx), None);
}

/// The search field's "Search history" follows a switch between light and dark while the Library
/// stays open, in the placeholder gray of the palette it's drawn in: a field that kept the gray of
/// the look it opened in drew Paper on a light window's field, almost the field's own colour.
#[gpui::test]
fn the_search_placeholder_follows_the_appearance(cx: &mut TestAppContext) {
    use crate::shell::Appearance;
    use crate::tokens::shell::library::SEARCH_PLACEHOLDER;
    let (shell, _, cx) = history(Vec::new(), cx);
    let mut colours = Vec::new();
    for appearance in [Appearance::Light, Appearance::Dark, Appearance::Light] {
        shell.update(cx, |shell, cx| {
            shell.run(Command::Appearance(appearance), "ui", cx)
        });
        cx.run_until_parked();
        let (drawn, meant) = shell.read_with(cx, |shell, cx| {
            let field = shell.library.as_ref().expect("the Library").search.read(cx);
            (
                field.placeholder_hsla(),
                shell.palette.gray(SEARCH_PLACEHOLDER),
            )
        });
        assert_eq!(drawn, meant, "{appearance:?}'s placeholder");
        colours.push(drawn);
    }
    assert_ne!(
        colours[0], colours[1],
        "light and dark drew the same placeholder"
    );
    assert_eq!(colours[0], colours[2]);
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
        sorted: None,
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

/// A task is listed under the day it ended, whenever it left the foot, so the time its row says
/// is its heading's day's, as the pages' rows around it: last night's task is under Yesterday
/// with its time alone, though the user dismissed it this morning.
#[gpui::test]
fn a_task_is_listed_under_the_day_it_ended(cx: &mut TestAppContext) {
    let minute = 5. * 60. * 1000.;
    let (shell, host, cx) = history(vec![closed(5, "news", at(13, 9), false)], cx);
    host.day_starts
        .set([at(13, 0), at(12, 0), at(9, 0), at(2, 0)]);
    let last_night = super::ArchivedTask {
        ended: at(12, 23) + minute,
        ..task()
    };
    shell.update(cx, |shell, cx| {
        shell.set_archived_tasks(vec![last_night], cx)
    });
    cx.run_until_parked();
    let listed = shell.read_with(cx, |shell, _| {
        let library = shell.library.as_ref().unwrap();
        let named = library.items.iter().map(|item| match item {
            Item::Heading(heading) => heading.to_string(),
            Item::Day(day) => super::day_heading(*day),
            Item::Task(index) => shell.archived_task_about(&shell.archived_tasks[*index]),
            _ => "a tab".to_string(),
        });
        named.collect::<Vec<_>>()
    });
    let time = match crate::clock::twelve_hour() {
        true => "11:05 PM",
        false => "23:05",
    };
    let row = format!("Claude · Done {time} · 1 page");
    assert_eq!(listed, ["Today", "a tab", "Yesterday", row.as_str()]);
}

/// What the Library's row ring is on, as `items` names it.
fn ringed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    let line = shell
        .read_with(cx, |shell, _| shell.library.as_ref()?.keyed_row)?
        .0;
    items(shell, cx).get(line).cloned()
}

/// ↓ and ↑ from History's search field ring its rows as the command bar's keys go over its
/// suggestions: past the day headings, wrapping at the ends, the ringed row an option of the list,
/// selected, and said with where it stands. Return opens it as a click does; typing ends the ring.
#[gpui::test]
fn the_arrows_ring_historys_rows(cx: &mut TestAppContext) {
    use gpui::Focusable;
    let (shell, host, cx) = history(vec![closed(5, "news", at(9, 15), false)], cx);
    cx.update(|_, cx| crate::shell::commands::bind_keys(cx));
    cx.simulate_a11y_activation();
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let pages = vec![page("mail", at(9, 14)), page("docs", at(8, 10))];
    answer(&shell, first, pages, false, cx);
    made(&host);
    let search = shell.read_with(cx, |shell, cx| {
        shell.library.as_ref().unwrap().search.focus_handle(cx)
    });
    cx.update(|window, cx| window.focus(&search, cx));
    cx.run_until_parked();
    assert_eq!(ringed(&shell, cx), None);
    let mut went = Vec::new();
    for key in ["down", "down", "down", "down", "up"] {
        cx.simulate_keystrokes(key);
        went.push(ringed(&shell, cx).unwrap_or_default());
    }
    let went: Vec<&str> = went.iter().map(String::as_str).collect();
    assert_eq!(
        went,
        [
            "closed news",
            "visited mail",
            "visited docs",
            "closed news",
            "visited docs"
        ]
    );

    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let node = |role: gpui::Role, label: &str| {
        (tree.nodes.iter())
            .map(|(_, node)| node)
            .find(|node| node.role() == role && node.label().as_deref() == Some(label))
            .cloned()
    };
    let option = node(gpui::Role::ListBoxOption, "docs").expect("docs's row is an option");
    assert_eq!(option.is_selected(), Some(true));
    let news = node(gpui::Role::ListBoxOption, "news").expect("news's row is an option");
    assert_ne!(news.is_selected(), Some(true));
    assert!(
        node(gpui::Role::Status, "docs, 3 of 3").is_some(),
        "the live region didn't say the ringed row"
    );

    cx.simulate_keystrokes("up enter");
    let made = made(&host);
    assert_eq!(made.len(), 1, "made {made:?}");
    let peek = format!("open preview over 10 {} from ", url("mail"));
    assert!(made[0].starts_with(&peek), "made {made:?}");
    assert_eq!(
        ringed(&shell, cx),
        None,
        "the ring outlived its row's opening"
    );

    cx.simulate_keystrokes("down");
    assert!(ringed(&shell, cx).is_some());
    cx.simulate_input("m");
    assert_eq!(ringed(&shell, cx), None, "typing left the ring on a row");
}

/// What ⌘Z, ⌘F and ⌘W each did, as History's field holds the keyboard after `before`: the calls
/// to the browser, whether the Library stayed open and what has the keyboard.
fn window_shortcuts(before: &str, cx: &mut TestAppContext) -> Vec<String> {
    use gpui::Focusable;
    let (shell, host, cx) = history(vec![closed(5, "news", at(9, 15), false)], cx);
    cx.update(|_, cx| crate::shell::commands::bind_keys(cx));
    next_frame(cx);
    let (first, _) = one_ask(&host);
    answer(&shell, first, vec![page("mail", at(9, 14))], false, cx);
    made(&host);
    let search = shell.read_with(cx, |shell, cx| {
        shell.library.as_ref().unwrap().search.focus_handle(cx)
    });
    cx.update(|window, cx| window.focus(&search, cx));
    cx.run_until_parked();
    if before == "down" {
        cx.simulate_keystrokes(before);
        assert!(ringed(&shell, cx).is_some(), "{before} rang no row");
    } else if !before.is_empty() {
        cx.simulate_keystrokes(before);
    }
    let mut did = Vec::new();
    for key in ["cmd-z", "cmd-f", "cmd-w"] {
        cx.simulate_keystrokes(key);
        let open = shell.read_with(cx, |shell, _| shell.library_open);
        let field = cx.update(|window, _| search.is_focused(window));
        did.push(format!(
            "{key}: {:?}, Library open {open}, field focused {field}",
            made(&host)
        ));
    }
    did
}

/// The ring takes only ↓, ↑ and Return: the window's shortcuts do with a row ringed what they do
/// without one.
#[gpui::test]
fn the_rows_ring_leaves_the_windows_shortcuts_as_they_are(cx: &mut TestAppContext) {
    let without = window_shortcuts("", cx);
    let with = window_shortcuts("down", cx);
    assert_eq!(with, without);
}

/// Tab from the field to the Library's sections moves the keyboard and nothing else: the window's
/// shortcuts make the same calls and leave the Library as they do from the field.
#[gpui::test]
fn a_section_stop_leaves_the_windows_shortcuts_as_they_are(cx: &mut TestAppContext) {
    let calls = |did: Vec<String>| -> Vec<String> {
        did.into_iter()
            .map(|each| each.split(", field focused").next().unwrap().to_owned())
            .collect()
    };
    let without = window_shortcuts("", cx);
    let with = window_shortcuts("tab", cx);
    assert!(
        with.iter()
            .all(|each| each.ends_with("field focused false")),
        "Tab left the field"
    );
    assert_eq!(calls(with), calls(without));
}

/// A ringed row removed by code, here a closed tab reopened elsewhere, gives the ring to the row
/// that took its place, as one the user closes does (#4108).
#[gpui::test]
fn a_ringed_row_removed_gives_the_ring_to_the_row_in_its_place(cx: &mut TestAppContext) {
    let (shell, host, cx) = history(vec![closed(5, "news", at(9, 15), false)], cx);
    cx.update(|_, cx| crate::shell::commands::bind_keys(cx));
    next_frame(cx);
    let (first, _) = one_ask(&host);
    let pages = vec![page("mail", at(9, 14)), page("docs", at(8, 10))];
    answer(&shell, first, pages, false, cx);
    let search = shell.read_with(cx, |shell, cx| {
        gpui::Focusable::focus_handle(&*shell.library.as_ref().unwrap().search.read(cx), cx)
    });
    cx.update(|window, cx| window.focus(&search, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("down");
    assert_eq!(ringed(&shell, cx).as_deref(), Some("closed news"));
    shell.update(cx, |shell, cx| shell.set_archive(Vec::new(), cx));
    cx.run_until_parked();
    assert_eq!(ringed(&shell, cx).as_deref(), Some("visited mail"));
}
