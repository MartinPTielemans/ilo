//! A tab of the user's, handed to a thread: only the user's send hands one, each whole or not at
//! all, and a thread learns of a tab only what it would of a page of its own. The assistant is
//! the stand-in of kept.rs, on the made-up browser.

use super::kept::Run;
use super::*;
use crate::agents::runner::Cli;
use crate::agents::task::Start;
use crate::agents::task::handed::{self, Picked};

/// The user's Space, and a thread's, the first the browser makes.
const USERS: u64 = 1;
const THREADS: u64 = 100;
/// Two tabs of the user's on the web, in the Space the window shows.
const TRAINS: (u64, &str, &str) = (11, "Trains to Lisbon", "https://www.example.com/trains");
const SEATS: (u64, &str, &str) = (12, "Seats", "https://tickets.example/seats");
const ASKED: &str = "Book the 9:05";

/// A run with the user's two tabs open.
fn run(name: &str, cx: &mut TestAppContext) -> Run {
    let run = Run::new(name, cx);
    for (tab, title, url) in [TRAINS, SEATS] {
        run.browser.tabs.borrow_mut().push((tab, USERS, title, url));
    }
    run
}

/// The user sends `asked` with the chips of `tabs`, picked in the Space `space`.
fn send(
    run: &Run,
    asked: &str,
    space: u64,
    tabs: &[u64],
    cx: &mut TestAppContext,
) -> Option<String> {
    let start = Start {
        tabs: Picked::of_chips(space, tabs.to_vec()),
        ..Start::default()
    };
    let started = (run.agents).update(cx, |agents, cx| {
        agents.start_task(Cli::Claude, asked, None, USERS, start, cx)
    });
    cx.run_until_parked();
    started
}

/// The Space `tab` is in, or 0 for one that isn't open.
fn space_of(run: &Run, tab: u64) -> u64 {
    let tabs = run.browser.tabs.borrow();
    tabs.iter()
        .find(|each| each.0 == tab)
        .map_or(0, |each| each.1)
}

/// The tabs the core was asked to give and gave, in order.
fn given(run: &Run) -> Vec<u64> {
    let given = run.browser.given.borrow();
    given.iter().map(|each| each.0).collect()
}

/// The sites of the entries of tabs handed to `agent`'s thread, oldest first.
fn entries(run: &Run, agent: &str, action: &str, cx: &mut TestAppContext) -> Vec<String> {
    let mut sites: Vec<String> = run.agents.read_with(cx, |agents, _| {
        (agents.activity(agent))
            .filter(|entry| entry.action.as_ref() == action)
            .map(|entry| entry.site.to_string())
            .collect()
    });
    // The log gives the newest first.
    sites.reverse();
    sites
}

fn listed(run: &Run, agent: &str, cx: &mut TestAppContext) -> String {
    let answer = call(&run.agents, agent, "list_tabs", json!({}), cx);
    answer.try_recv().expect("list_tabs answers at once").text
}

/// The tabs the user picked are the thread's pages from the send: given by the core in the chips'
/// order, each with its entry by its host alone, and its assistant is told how many and each as
/// list_tabs lists it, after the user's words.
#[gpui::test]
fn the_tabs_picked_are_the_threads_pages_from_the_send(cx: &mut TestAppContext) {
    let run = run("handed-send", cx);
    let agent = send(&run, ASKED, USERS, &[SEATS.0, TRAINS.0], cx).expect("started");
    assert_eq!(given(&run), [SEATS.0, TRAINS.0]);
    for tab in [SEATS.0, TRAINS.0] {
        assert_eq!(space_of(&run, tab), THREADS);
        assert!(
            run.agents
                .read_with(cx, |agents, _| agents.is_handed(&agent, tab))
        );
    }
    assert_eq!(
        entries(&run, &agent, handed::HANDED, cx),
        ["tickets.example", "example.com"]
    );
    let lines = format!(
        "  {}  {}  {}\n  {}  {}  {}",
        SEATS.0, SEATS.1, SEATS.2, TRAINS.0, TRAINS.1, TRAINS.2
    );
    let (_, first) = run.ran(1);
    assert_eq!(
        first,
        format!("{ASKED}\n\nThe user added 2 of their tabs to your pages.\n{lines}")
    );
    // Its pages as any of its own, and nothing else of the user's.
    let list = listed(&run, &agent, cx);
    assert!(list.contains(SEATS.2) && list.contains(TRAINS.2), "{list}");
    assert!(!list.contains("wikipedia"), "{list}");
}

/// A pick is only a pick: until its message is sent the core hears of no tab, and a thread
/// started without it has none of the user's tabs.
#[gpui::test]
fn a_pick_never_sent_hands_nothing(cx: &mut TestAppContext) {
    let run = run("handed-unsent", cx);
    let picked = Picked::of_chips(USERS, vec![TRAINS.0]);
    assert!(!picked.is_empty());
    drop(picked);
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    assert!(given(&run).is_empty());
    assert!(entries(&run, &agent, handed::HANDED, cx).is_empty());
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert_eq!(run.ran(1).1, ASKED);
    assert!(!listed(&run, &agent, cx).contains(TRAINS.2));
}

/// A tab picked and no words starts a thread, whose first message is what its assistant is told
/// of the tab and nothing else. No words and no tab that goes starts none, and leaves nothing.
#[gpui::test]
fn a_tab_and_no_words_starts_a_thread(cx: &mut TestAppContext) {
    let run = run("handed-alone", cx);
    assert_eq!(send(&run, "  ", USERS, &[], cx), None);
    // One the core won't give, which the app can't know before it asks.
    run.browser.not_given.borrow_mut().insert(SEATS.0);
    assert_eq!(send(&run, "", USERS, &[SEATS.0], cx), None);
    assert!(given(&run).is_empty());
    assert!(
        run.agents
            .read_with(cx, |agents, _| agents.tasks().next().is_none())
    );
    assert_eq!(run.browser.task_spaces(), [0u64; 0]);

    let agent = send(&run, "", USERS, &[TRAINS.0], cx).expect("started");
    assert_eq!(given(&run), [TRAINS.0]);
    assert_eq!(
        run.ran(1).1,
        format!(
            "The user added one of their tabs to your pages.\n  {}  {}  {}",
            TRAINS.0, TRAINS.1, TRAINS.2
        )
    );
    assert_eq!(entries(&run, &agent, handed::HANDED, cx), ["example.com"]);
}

/// Only a tab of the user's own, on the web, in the Space the window showed at the send, is
/// handed: not one of another Space, of an agent's, of a private window or of another profile,
/// not a page off the web, and not one the core refuses. Each that isn't handed leaves no entry
/// and nothing its assistant is told, and the others picked with it still go.
#[gpui::test]
fn a_tab_that_isnt_handed_leaves_nothing_and_the_others_go(cx: &mut TestAppContext) {
    let run = run("handed-refused", cx);
    const ELSEWHERE: u64 = 3;
    const PRIVATE: u64 = 5;
    const OTHER_PROFILE: u64 = 7;
    run.browser.tabs.borrow_mut().extend([
        (13, ELSEWHERE, "Notes", "https://notes.example/"),
        (15, PRIVATE, "Bank", "https://bank.example/"),
        (16, USERS, "Settings", "chrome://settings/"),
        (17, OTHER_PROFILE, "Work mail", "https://mail.example/"),
        (18, USERS, "Upload", "https://files.example/"),
    ]);
    run.browser.private.borrow_mut().push(PRIVATE);
    (run.browser.profiles.borrow_mut()).push((OTHER_PROFILE, "Profile 2"));
    run.browser.not_given.borrow_mut().insert(18);
    // The Space in view is the user's: a tab elsewhere, the outside agent's own and one not open.
    for (space, tab) in [
        (USERS, 13),
        (USERS, 20),
        (2, 20),
        (USERS, 99),
        (PRIVATE, 15),
        (OTHER_PROFILE, 17),
        (USERS, 16),
        (USERS, 18),
    ] {
        let before = space_of(&run, tab);
        let agent = send(&run, ASKED, space, &[tab], cx).expect("its words start it");
        assert_eq!(given(&run), [0u64; 0], "tab {tab} from {space}");
        assert_eq!(space_of(&run, tab), before);
        assert!(entries(&run, &agent, handed::HANDED, cx).is_empty());
        assert!(
            !run.agents
                .read_with(cx, |agents, _| agents.is_handed(&agent, tab))
        );
        let told = run.agents.read_with(cx, |agents, _| {
            agents.task(&agent).expect("its task").asked.to_string()
        });
        assert_eq!(told, ASKED);
    }
    let runs = run.agents.read_with(cx, |agents, _| agents.tasks().count());
    let agent = send(&run, ASKED, USERS, &[16, TRAINS.0, 18], cx).expect("started");
    assert_eq!(given(&run), [TRAINS.0]);
    assert_eq!(entries(&run, &agent, handed::HANDED, cx), ["example.com"]);
    let (_, first) = run.ran(runs + 1);
    assert_eq!(
        first,
        format!(
            "{ASKED}\n\nThe user added one of their tabs to your pages.\n  {}  {}  {}",
            TRAINS.0, TRAINS.1, TRAINS.2
        )
    );
}

/// What a pick shows of itself is how many tabs it has, so no log line or failed test names one.
#[test]
fn a_pick_shows_only_its_count() {
    let picked = Picked::of_chips(USERS, vec![TRAINS.0, SEATS.0, TRAINS.0]);
    assert_eq!(format!("{picked:?}"), "Picked(2)");
    assert_eq!(format!("{:?}", Picked::default()), "Picked(0)");
}

/// The entry of a handed tab reads as its whole sentence wherever an entry is read, by its host.
#[test]
fn a_handed_tabs_entry_is_its_whole_sentence() {
    let did = tools::Did {
        tool: handed::HANDED.into(),
        site: "www.example.com".into(),
        ..Default::default()
    };
    for tense in [tools::Tense::Done, tools::Tense::Failed] {
        assert_eq!(
            tools::step_title(&did, tense),
            "You moved www.example.com to the thread"
        );
    }
    assert_eq!(handed::entry("open_tab", "www.example.com"), None);
}
