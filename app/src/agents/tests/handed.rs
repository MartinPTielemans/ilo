//! A tab of the user's, handed to a thread: only the user's send hands one, each whole or not at
//! all, and a thread learns of a tab only what it would of a page of its own. The assistant is
//! the stand-in of kept.rs, on the made-up browser.

use super::kept::Run;
use super::*;
use crate::agents::runner::{Cli, Out};
use crate::agents::task::handed::{self, Picked};
use crate::agents::task::{Start, State, TaskAction};
use crate::agents::tier::Sites;

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
    send_with(run, asked, start, cx)
}

fn send_with(run: &Run, asked: &str, start: Start, cx: &mut TestAppContext) -> Option<String> {
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

/// The tabs of `space`, in their order.
fn tabs_of(run: &Run, space: u64) -> Vec<u64> {
    let tabs = run.browser.tabs.borrow();
    (tabs.iter().filter(|each| each.1 == space))
        .map(|each| each.0)
        .collect()
}

fn is_handed(run: &Run, agent: &str, tab: u64, cx: &mut TestAppContext) -> bool {
    run.agents
        .read_with(cx, |agents, _| agents.is_handed(agent, tab))
}

/// The user's press of Return to Your Tabs on `tab` in `agent`'s thread.
fn press_return(run: &Run, agent: &str, tab: u64, cx: &mut TestAppContext) {
    run.agents.update(cx, |agents, cx| {
        agents.task_action(agent, TaskAction::ReturnTab(tab), cx);
    });
    cx.run_until_parked();
}

/// What `tool` on `tab` is answered at once.
fn answered(run: &Run, agent: &str, tool: &str, tab: u64, cx: &mut TestAppContext) -> ToolResult {
    let arguments = json!({ "tab": tab, "ref": "e1", "reason": "To carry on." });
    let answer = call(&run.agents, agent, tool, arguments, cx);
    cx.run_until_parked();
    answer.try_recv().expect("answered at once")
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
    let mut runs = 0;
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
        // Each run is waited for, as the stand-in numbers its runs as they start.
        runs += 1;
        assert_eq!(run.ran(runs).1, ASKED);
    }
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
    let did = tools::Did {
        tool: handed::TAKEN_BACK.into(),
        site: "example.com".into(),
        ..Default::default()
    };
    assert_eq!(
        tools::step_title(&did, tools::Tense::Done),
        "You returned example.com to your tabs"
    );
    let did = tools::Did {
        tool: handed::GIVEN_BACK.into(),
        site: "example.com".into(),
        ..Default::default()
    };
    assert_eq!(
        tools::step_title(&did, tools::Tense::Done),
        "example.com returned to your tabs"
    );
    assert_eq!(handed::entry("open_tab", "www.example.com"), None);
}

/// Return to Your Tabs takes a handed tab back at once, mid-step: the call under way on it hears
/// the tab is the user's again, the core puts it back where it left, its entry says the user did
/// it, and from then every call that names it, an ask for it too, hears the same and reaches no
/// page. The thread's other tabs stay its own.
#[gpui::test]
fn return_to_your_tabs_takes_the_tab_back_mid_step(cx: &mut TestAppContext) {
    let run = run("handed-return", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
    let reading = call(
        &run.agents,
        &agent,
        "read_page",
        json!({ "tab": TRAINS.0 }),
        cx,
    );
    cx.run_until_parked();
    assert!(reading.try_recv().is_err(), "the read is under way");
    let reads = run.browser.reads.borrow().len();

    press_return(&run, &agent, TRAINS.0, cx);
    let answer = reading.try_recv().expect("the read under way is answered");
    assert!(answer.error);
    assert_eq!(answer.text, handed::RETURNED);
    assert_eq!(*run.browser.returned.borrow(), [TRAINS.0]);
    assert_eq!(tabs_of(&run, USERS), [10, TRAINS.0]);
    assert!(!is_handed(&run, &agent, TRAINS.0, cx));
    assert_eq!(
        entries(&run, &agent, handed::TAKEN_BACK, cx),
        ["example.com"]
    );
    for tool in ["read_page", "click", "snapshot", "ask_for_tab", "close_tab"] {
        let answer = answered(&run, &agent, tool, TRAINS.0, cx);
        assert!(answer.error, "{tool}: {answer:?}");
        assert_eq!(answer.text, handed::RETURNED, "{tool}");
    }
    assert_eq!(run.browser.reads.borrow().len(), reads);
    assert!(run.browser.pages.borrow().is_empty(), "the core acted");
    assert_eq!(tabs_of(&run, USERS), [10, TRAINS.0]);
    assert!(!listed(&run, &agent, cx).contains(TRAINS.2));
    // The other is the thread's still, and comes back to its own place when its turn comes.
    assert!(is_handed(&run, &agent, SEATS.0, cx));
    assert_eq!(space_of(&run, SEATS.0), THREADS);
    // A second press on a tab that's the user's does nothing.
    press_return(&run, &agent, TRAINS.0, cx);
    assert_eq!(*run.browser.returned.borrow(), [TRAINS.0]);
    assert_eq!(entries(&run, &agent, handed::TAKEN_BACK, cx).len(), 1);
    press_return(&run, &agent, SEATS.0, cx);
    assert_eq!(space_of(&run, SEATS.0), USERS);
    assert!(tabs_of(&run, THREADS).is_empty());
    assert_eq!(
        entries(&run, &agent, handed::TAKEN_BACK, cx),
        ["example.com", "tickets.example"]
    );
}

/// Only a tab the thread holds of the user's is returned: a page it opened itself, a tab of the
/// user's it never had and a tab handed to another thread are left as they are, with no entry.
#[gpui::test]
fn only_a_handed_tab_is_returned(cx: &mut TestAppContext) {
    let run = run("handed-return-own", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    let other = send(&run, ASKED, USERS, &[SEATS.0], cx).expect("started");
    let opened = call(
        &run.agents,
        &agent,
        "open_tab",
        json!({ "url": "https://shop.example/" }),
        cx,
    );
    cx.run_until_parked();
    drop(opened);
    let own = *(tabs_of(&run, THREADS).iter())
        .find(|tab| **tab != TRAINS.0)
        .expect("the page it opened");
    for tab in [own, 10, SEATS.0, 99] {
        let before = space_of(&run, tab);
        press_return(&run, &agent, tab, cx);
        assert_eq!(space_of(&run, tab), before, "tab {tab}");
    }
    assert!(run.browser.returned.borrow().is_empty());
    assert!(entries(&run, &agent, handed::TAKEN_BACK, cx).is_empty());
    assert!(entries(&run, &other, handed::TAKEN_BACK, cx).is_empty());
    assert!(is_handed(&run, &other, SEATS.0, cx));
}

/// A handed tab the user took over and handed back is the thread's still. Returned while they
/// hold it, the hold ends with it: the call that waited on it hears the tab is the user's, the
/// return's entry is the one entry, and nothing is handed back to the thread.
#[gpui::test]
fn a_tab_the_user_holds_goes_back_with_its_hold(cx: &mut TestAppContext) {
    let run = run("handed-return-held", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
    let held = |cx: &mut TestAppContext, tab: u64| {
        run.agents
            .read_with(cx, |agents, _| agents.held.contains_key(&tab))
    };
    (run.agents).update(cx, |agents, cx| agents.take_over(&agent, TRAINS.0, cx));
    assert!(held(cx, TRAINS.0));
    run.agents
        .update(cx, |agents, cx| agents.hand_back(TRAINS.0, cx));
    cx.run_until_parked();
    assert!(!held(cx, TRAINS.0) && is_handed(&run, &agent, TRAINS.0, cx));
    assert_eq!(space_of(&run, TRAINS.0), THREADS);
    assert!(run.browser.returned.borrow().is_empty());

    (run.agents).update(cx, |agents, cx| agents.take_over(&agent, TRAINS.0, cx));
    let asked = call(
        &run.agents,
        &agent,
        "ask_for_tab",
        json!({ "tab": TRAINS.0, "reason": "To book it." }),
        cx,
    );
    cx.run_until_parked();
    assert!(asked.try_recv().is_err(), "the ask waits on the user");
    let before = run
        .agents
        .read_with(cx, |agents, _| agents.activity(&agent).count());
    press_return(&run, &agent, TRAINS.0, cx);
    assert!(!held(cx, TRAINS.0));
    assert_eq!(asked.try_recv().expect("answered").text, handed::RETURNED);
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    let after = run
        .agents
        .read_with(cx, |agents, _| agents.activity(&agent).count());
    assert_eq!(after, before + 1, "the return's entry and no other");

    // A step the thread handed to the user, on its other tab.
    let handed_off = call(
        &run.agents,
        &agent,
        "hand_off",
        json!({ "tab": SEATS.0, "reason": "Sign in." }),
        cx,
    );
    cx.run_until_parked();
    assert!(handed_off.try_recv().is_err(), "the hand-off waits");
    press_return(&run, &agent, SEATS.0, cx);
    assert_eq!(
        handed_off.try_recv().expect("answered").text,
        handed::RETURNED
    );
    assert!(!held(cx, SEATS.0));
    assert_eq!(space_of(&run, SEATS.0), USERS);
}

/// A handed tab the user moved to a Space of theirs by another road is returned as by the press,
/// with the core not asked again; one that closed is no longer the thread's, and has no return.
#[gpui::test]
fn a_handed_tab_that_left_by_another_road_is_the_users(cx: &mut TestAppContext) {
    let run = run("handed-left", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
    for each in run.browser.tabs.borrow_mut().iter_mut() {
        if each.0 == TRAINS.0 {
            each.1 = USERS;
        }
    }
    (run.agents).update(cx, |agents, cx| agents.space_changed(THREADS, cx));
    cx.run_until_parked();
    assert!(!is_handed(&run, &agent, TRAINS.0, cx));
    assert!(run.browser.returned.borrow().is_empty());
    assert_eq!(
        entries(&run, &agent, handed::TAKEN_BACK, cx),
        ["example.com"]
    );
    assert_eq!(
        answered(&run, &agent, "read_page", TRAINS.0, cx).text,
        handed::RETURNED
    );
    assert!(is_handed(&run, &agent, SEATS.0, cx));

    (run.browser.tabs.borrow_mut()).retain(|each| each.0 != SEATS.0);
    run.agents.update(cx, |agents, cx| {
        agents.space_changed(THREADS, cx);
        agents.tab_closed(SEATS.0, cx);
    });
    cx.run_until_parked();
    assert!(!is_handed(&run, &agent, SEATS.0, cx));
    assert_eq!(entries(&run, &agent, handed::TAKEN_BACK, cx).len(), 1);
    assert!(run.browser.returned.borrow().is_empty());
}

/// Where the core can't put a tab back, as when the Space it left is gone, the return still takes
/// it from the thread: it's a tab of a Space of the user's.
#[gpui::test]
fn a_tab_the_core_cant_put_back_is_still_the_users(cx: &mut TestAppContext) {
    let run = run("handed-return-nowhere", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    run.browser.not_returned.borrow_mut().insert(TRAINS.0);
    press_return(&run, &agent, TRAINS.0, cx);
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert!(!is_handed(&run, &agent, TRAINS.0, cx));
    assert_eq!(
        entries(&run, &agent, handed::TAKEN_BACK, cx),
        ["example.com"]
    );
}

/// A thread's close of a handed tab gives it back: no close reaches the core, the tab is at its
/// place among the user's on the page it's at, the entry says it returned, and the thread hears
/// what its close of a page of its own says, then and after. A page it opened closes as ever.
#[gpui::test]
fn a_threads_close_of_a_handed_tab_gives_it_back(cx: &mut TestAppContext) {
    let run = run("handed-close", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
    let opened = call(
        &run.agents,
        &agent,
        "open_tab",
        json!({ "url": "https://shop.example/" }),
        cx,
    );
    cx.run_until_parked();
    drop(opened);
    let own = *(tabs_of(&run, THREADS).iter())
        .find(|tab| ![TRAINS.0, SEATS.0].contains(tab))
        .expect("the page it opened");

    let closed = answered(&run, &agent, "close_tab", TRAINS.0, cx);
    assert!(!closed.error, "{closed:?}");
    assert_eq!(closed.text, format!("Closed tab {}.", TRAINS.0));
    // Discarding loses nothing either: the page goes back as it is.
    let discard = json!({ "tab": SEATS.0, "discard": true });
    let closed = call(&run.agents, &agent, "close_tab", discard, cx);
    cx.run_until_parked();
    let closed = closed.try_recv().expect("answered at once");
    assert_eq!(closed.text, format!("Closed tab {}.", SEATS.0));
    assert!(
        run.browser.closes.borrow().is_empty(),
        "the core closed one"
    );
    assert_eq!(*run.browser.returned.borrow(), [TRAINS.0, SEATS.0]);
    for tab in [TRAINS.0, SEATS.0] {
        assert_eq!(space_of(&run, tab), USERS);
        assert!(!is_handed(&run, &agent, tab, cx));
    }
    assert_eq!(
        entries(&run, &agent, handed::GIVEN_BACK, cx),
        ["example.com", "tickets.example"]
    );
    assert!(entries(&run, &agent, handed::TAKEN_BACK, cx).is_empty());
    let later = answered(&run, &agent, "read_page", TRAINS.0, cx);
    assert!(later.error);
    assert!(
        later
            .text
            .starts_with(&format!("The agent closed tab {}.", TRAINS.0)),
        "{later:?}"
    );

    let closed = answered(&run, &agent, "close_tab", own, cx);
    assert_eq!(closed.text, format!("Closed tab {own}."));
    assert_eq!(space_of(&run, own), 0);
    assert_eq!(entries(&run, &agent, handed::GIVEN_BACK, cx).len(), 2);
}

/// remove_tab on a handed tab its agent pinned gives it back as its close does, with the close's
/// answer: it's unpinned for the user and never closed.
#[gpui::test]
fn a_remove_of_a_handed_tab_gives_it_back(cx: &mut TestAppContext) {
    let run = run("handed-remove", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    let pin = |cx: &mut TestAppContext, action: &'static str| {
        let (reply, answer) = mpsc::channel();
        run.agents.update(cx, |agents, cx| {
            agents.pin(&agent, 0, action, TRAINS.0, reply, cx)
        });
        cx.run_until_parked();
        answer.try_recv().expect("answered at once")
    };
    assert!(!pin(cx, "pin_tab").error);
    let removed = pin(cx, "remove_tab");
    assert!(!removed.error, "{removed:?}");
    assert_eq!(removed.text, format!("Closed tab {}.", TRAINS.0));
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert_eq!(*run.browser.returned.borrow(), [TRAINS.0]);
    assert_eq!(
        entries(&run, &agent, handed::GIVEN_BACK, cx),
        ["example.com"]
    );
}

/// The user's own close of a tab a thread holds of theirs returns it instead: it's theirs again,
/// open, and the entry says they returned it. Any other tab is theirs to close as ever.
#[gpui::test]
fn the_users_close_of_a_handed_tab_returns_it(cx: &mut TestAppContext) {
    let run = run("handed-user-close", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    let closes = |cx: &mut TestAppContext, tab: u64| {
        (run.agents).update(cx, |agents, cx| agents.close_returns(tab, cx))
    };
    assert!(!closes(cx, SEATS.0) && !closes(cx, 10) && !closes(cx, 99));
    assert!(run.browser.returned.borrow().is_empty());
    assert!(closes(cx, TRAINS.0));
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert_eq!(
        entries(&run, &agent, handed::TAKEN_BACK, cx),
        ["example.com"]
    );
    assert_eq!(
        answered(&run, &agent, "read_page", TRAINS.0, cx).text,
        handed::RETURNED
    );
    assert!(!closes(cx, TRAINS.0));
}

/// Each way a thread stops being under way returns every tab it holds of the user's first: its
/// answer, finish, Stop, a failure, its plan's limit, Dismiss and a quit, with or without a
/// window left. Nothing closes one, none goes with the thread's Space, and the thread's own
/// record has each return's entry.
#[gpui::test]
fn a_thread_no_longer_under_way_holds_no_tab_of_the_users(cx: &mut TestAppContext) {
    use crate::agents::runner::Failure;
    type End = fn(&Run, &str, &mut TestAppContext);
    let ends: [(&str, End); 8] = [
        ("answer", |run, agent, cx| run.finish(agent, cx)),
        ("finish", |run, agent, cx| {
            let pages = json!({ "result": "Booked.", "tabs": [TRAINS.0] });
            let done = call(&run.agents, agent, "finish", pages, cx);
            cx.run_until_parked();
            assert!(!done.try_recv().expect("answered").error);
            // A tab of the user's is none of the pages the thread leaves them.
            let left = match run.state(agent, cx) {
                State::Done { tabs, .. } => tabs,
                state => panic!("not done: {state:?}"),
            };
            assert_eq!(left, [0u64; 0]);
        }),
        ("stop", |run, agent, cx| {
            (run.agents).update(cx, |agents, cx| agents.stop_task(agent, cx))
        }),
        ("failure", |run, agent, cx| {
            let failure = Failure::Other("It broke.".into());
            (run.agents).update(cx, |agents, cx| agents.fail(agent, failure, cx))
        }),
        ("limit", |run, agent, cx| {
            (run.agents).update(cx, |agents, cx| {
                agents.fail(agent, Failure::Limit(None), cx)
            })
        }),
        ("dismiss", |run, agent, cx| {
            (run.agents).update(cx, |agents, cx| agents.dismiss_task(agent, cx))
        }),
        ("quit", |run, _, cx| {
            (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx))
        }),
        ("quit-no-window", |run, _, cx| {
            (run.agents).update(cx, |agents, cx| agents.end_tasks(false, cx))
        }),
    ];
    for (name, end) in ends {
        let run = run(&format!("handed-end-{name}"), cx);
        let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
        end(&run, &agent, cx);
        cx.run_until_parked();
        assert_eq!(
            *run.browser.returned.borrow(),
            [TRAINS.0, SEATS.0],
            "{name}"
        );
        for tab in [TRAINS.0, SEATS.0] {
            assert_eq!(space_of(&run, tab), USERS, "{name}");
            assert!(!is_handed(&run, &agent, tab, cx), "{name}");
        }
        assert!(run.browser.closes.borrow().is_empty(), "{name}");
        assert_eq!(
            entries(&run, &agent, handed::GIVEN_BACK, cx),
            ["example.com", "tickets.example"],
            "{name}"
        );
        assert!(entries(&run, &agent, handed::TAKEN_BACK, cx).is_empty());
        // In the record of the thread that ended, which keeps what came before its end.
        run.agents.read_with(cx, |agents, _| {
            let Some(task) = agents.task(&agent) else {
                return;
            };
            let given_back = (agents.log.iter())
                .filter(|entry| entry.action.as_ref() == handed::GIVEN_BACK)
                .all(|entry| entry.id < task.end_entry);
            assert!(given_back, "{name}: a return noted after its thread's end");
        });
    }
}

/// A wait on the user is still under way: a thread that asks them, or hands them a step, keeps
/// the tabs it holds of theirs, and so does one whose turn goes on.
#[gpui::test]
fn a_thread_that_waits_on_the_user_keeps_its_tabs(cx: &mut TestAppContext) {
    let run = run("handed-waits", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0, SEATS.0], cx).expect("started");
    run.said(&agent, Out::Said("Looking at the trains.".into()), cx);
    let handed_off = call(
        &run.agents,
        &agent,
        "hand_off",
        json!({ "tab": SEATS.0, "reason": "Sign in." }),
        cx,
    );
    let ask = json!({ "question": "Row 12 or row 14?", "answers": ["Row 12", "Row 14"] });
    let asked = call(&run.agents, &agent, "ask", ask, cx);
    cx.run_until_parked();
    assert!(handed_off.try_recv().is_err() && asked.try_recv().is_err());
    assert!(matches!(run.state(&agent, cx), State::Asking { .. }));
    assert!(run.browser.returned.borrow().is_empty());
    for tab in [TRAINS.0, SEATS.0] {
        assert!(is_handed(&run, &agent, tab, cx));
        assert_eq!(space_of(&run, tab), THREADS);
    }
    assert!(entries(&run, &agent, handed::GIVEN_BACK, cx).is_empty());
}

/// A third tab of the user's, on the site of TRAINS.
const FARES: (u64, &str, &str) = (14, "Fares", "https://www.example.com/fares");

/// The sites `agent`'s thread may use.
fn sites(run: &Run, agent: &str, cx: &mut TestAppContext) -> Sites {
    run.agents.read_with(cx, |agents, _| {
        agents.task(agent).expect("its task").sites.clone()
    })
}

fn domain(url: &str) -> String {
    crate::sort_by_site::made_up_site(url).expect("a site")
}

/// A thread limited to the pages the user adds, started with the page `added` and `tabs`.
fn limited(run: &Run, added: &[&str], tabs: &[u64], cx: &mut TestAppContext) -> String {
    run.browser
        .tabs
        .borrow_mut()
        .push((FARES.0, USERS, FARES.1, FARES.2));
    let start = Start {
        pages: added.iter().map(|url| url.to_string()).collect(),
        only_added: true,
        tabs: Picked::of_chips(USERS, tabs.to_vec()),
        ..Start::default()
    };
    send_with(run, ASKED, start, cx).expect("started")
}

/// A handed tab's site joins the sites its thread may use, that one site, for as long as the
/// thread holds a handed tab of it: it stays while a second is held and goes at the last return,
/// with no change of access in the log, as the user changed none.
#[gpui::test]
fn a_handed_tabs_site_is_its_threads_while_a_tab_of_it_is_held(cx: &mut TestAppContext) {
    let run = run("handed-site", cx);
    const ADDED: &str = "https://fieldbook.example/lakeside";
    let agent = limited(&run, &[ADDED], &[TRAINS.0, FARES.0], cx);
    let (added, handed) = (domain(ADDED), domain(TRAINS.2));
    let with = Sites::Only(vec![added.clone(), handed]);
    assert_eq!(sites(&run, &agent, cx), with);
    let access = |cx: &mut TestAppContext| {
        run.agents.read_with(cx, |agents, _| {
            (agents.activity(&agent))
                .filter(|entry| entry.action.as_ref() == crate::agents::access::ACCESS_ACTION)
                .count()
        })
    };
    let logged = access(cx);
    press_return(&run, &agent, TRAINS.0, cx);
    assert_eq!(sites(&run, &agent, cx), with, "a second tab of it is held");
    // Whoever returns the last: here the thread, by its close.
    assert!(!answered(&run, &agent, "close_tab", FARES.0, cx).error);
    assert_eq!(sites(&run, &agent, cx), Sites::Only(vec![added]));
    assert_eq!(access(cx), logged);
}

/// A site that was the thread's before the give stays its own after the return, a thread that
/// may use any site is as it was, and sites the user set themselves meanwhile stay as set.
#[gpui::test]
fn a_site_that_was_the_threads_before_stays(cx: &mut TestAppContext) {
    let run = run("handed-site-before", cx);
    const ADDED: &str = "https://www.example.com/timetable";
    let own = Sites::Only(vec![domain(ADDED)]);
    let agent = limited(&run, &[ADDED], &[TRAINS.0], cx);
    assert_eq!(sites(&run, &agent, cx), own);
    press_return(&run, &agent, TRAINS.0, cx);
    assert_eq!(sites(&run, &agent, cx), own);

    let any = send(&run, ASKED, USERS, &[SEATS.0], cx).expect("started");
    assert_eq!(sites(&run, &any, cx), Sites::Any);
    press_return(&run, &any, SEATS.0, cx);
    assert_eq!(sites(&run, &any, cx), Sites::Any);

    let set = limited(&run, &["https://fieldbook.example/"], &[FARES.0], cx);
    let users = Sites::Only(vec![domain(FARES.2), "notes.example".into()]);
    (run.agents).update(cx, |agents, cx| agents.set_sites(&set, users.clone(), cx));
    press_return(&run, &set, FARES.0, cx);
    assert_eq!(sites(&run, &set, cx), users);
}

/// A tab picked for a thread limited to the pages the user adds is a page they add: with no
/// address added beside it, the thread may use that tab's site and no other, and none once the
/// tab is back, by its thread's end too.
#[gpui::test]
fn a_tab_alone_limits_its_thread_to_its_site(cx: &mut TestAppContext) {
    let run = run("handed-site-alone", cx);
    let agent = limited(&run, &[], &[TRAINS.0], cx);
    assert_eq!(sites(&run, &agent, cx), Sites::Only(vec![domain(TRAINS.2)]));
    run.finish(&agent, cx);
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert_eq!(sites(&run, &agent, cx), Sites::Only(Vec::new()));
}

/// The user's reply to `agent`'s thread: `text` with the chips of `tabs`, picked in their Space.
fn reply(run: &Run, agent: &str, text: &str, tabs: &[u64], cx: &mut TestAppContext) {
    let reply = handed::Reply {
        text: text.to_string().into(),
        tabs: Picked::of_chips(USERS, tabs.to_vec()),
    };
    run.agents.update(cx, |agents, cx| {
        agents.task_action(agent, TaskAction::Reply(reply), cx);
    });
    cx.run_until_parked();
}

/// The message the stand-in's run `ran` was given as its `line`th, the one it started with being
/// the first.
fn heard(run: &Run, ran: usize, line: usize) -> String {
    let input = super::kept::beside(&run.program, &format!("in.{ran}"));
    for _ in 0..500 {
        let sent = std::fs::read_to_string(&input).unwrap_or_default();
        // Whole once its line has ended.
        if sent.lines().count() > line || (sent.lines().count() == line && sent.ends_with('\n')) {
            let said: Value = serde_json::from_str(sent.lines().nth(line - 1).unwrap()).unwrap();
            return said["message"]["content"].as_str().unwrap().to_string();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("run {ran} of the stand-in was given no message {line}");
}

/// The user's messages `agent`'s thread has after the one that started it.
fn replies(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<String> {
    run.agents.read_with(cx, |agents, _| {
        let messages = agents.task(agent).unwrap().messages.iter();
        (messages.filter(|message| message.who == crate::agents::task::Who::User))
            .map(|message| message.text.to_string())
            .collect()
    })
}

/// The chips of `agent`'s thread's messages: which message, then each tab with its title and
/// whether it went.
fn chips(
    run: &Run,
    agent: &str,
    cx: &mut TestAppContext,
) -> Vec<(usize, Vec<(u64, String, bool)>)> {
    run.agents.read_with(cx, |agents, _| {
        (agents.message_chips(agent).into_iter())
            .map(|(said, chips)| {
                let chips = chips.into_iter();
                let each = chips.map(|chip| (chip.tab, chip.title.to_string(), chip.handed));
                (said, each.collect())
            })
            .collect()
    })
}

fn line_of(tab: (u64, &str, &str)) -> String {
    format!("  {}  {}  {}", tab.0, tab.1, tab.2)
}

/// A reply hands the tabs picked for it as its assistant is given it: the thread is taken up,
/// the core gives each, the log has each, and the message says how many after the user's words,
/// each as list_tabs lists it. The message shows its chips, by the titles the tabs had.
#[gpui::test]
fn a_reply_hands_the_tabs_picked_for_it(cx: &mut TestAppContext) {
    let run = run("handed-reply", cx);
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    run.finish(&agent, cx);
    reply(&run, &agent, " And these two ", &[TRAINS.0, SEATS.0], cx);
    assert_eq!(given(&run), [TRAINS.0, SEATS.0]);
    assert_eq!(run.state(&agent, cx), State::Working);
    assert_eq!(
        entries(&run, &agent, handed::HANDED, cx),
        ["example.com", "tickets.example"]
    );
    assert_eq!(
        heard(&run, 1, 2),
        format!(
            "And these two\n\nThe user added 2 of their tabs to your pages.\n{}\n{}",
            line_of(TRAINS),
            line_of(SEATS)
        )
    );
    // The conversation has their words alone, and the chips beside them.
    assert_eq!(replies(&run, &agent, cx), ["And these two"]);
    let shown = vec![
        (TRAINS.0, TRAINS.1.to_string(), true),
        (SEATS.0, SEATS.1.to_string(), true),
    ];
    assert_eq!(chips(&run, &agent, cx), [(1, shown)]);
}

/// A tab picked and no words is a reply, which is what its assistant is told of the tab and
/// nothing else. No words and no tab that goes is none: the thread is as it was.
#[gpui::test]
fn a_tab_and_no_words_is_a_reply(cx: &mut TestAppContext) {
    let run = run("handed-reply-alone", cx);
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    run.finish(&agent, cx);
    let done = run.state(&agent, cx);
    // One the core won't give, which the app can't know before it asks; and one not open.
    run.browser.not_given.borrow_mut().insert(SEATS.0);
    reply(&run, &agent, "  ", &[], cx);
    reply(&run, &agent, "", &[SEATS.0, 77], cx);
    assert_eq!(run.state(&agent, cx), done);
    assert!(replies(&run, &agent, cx).is_empty());
    assert!(chips(&run, &agent, cx).is_empty());
    assert!(given(&run).is_empty());
    let resumed = run.agents.read_with(cx, |agents, _| {
        (agents.activity(&agent)).any(|entry| entry.action.as_ref() == "resumed")
    });
    assert!(!resumed, "a reply with nothing in it took the thread up");
    // One that goes, beside one that doesn't, of which nothing is said.
    reply(&run, &agent, "", &[SEATS.0, TRAINS.0], cx);
    assert_eq!(given(&run), [TRAINS.0]);
    assert_eq!(
        heard(&run, 1, 2),
        format!(
            "The user added one of their tabs to your pages.\n{}",
            line_of(TRAINS)
        )
    );
    assert_eq!(replies(&run, &agent, cx), [""]);
    let shown = vec![
        (SEATS.0, SEATS.1.to_string(), false),
        (TRAINS.0, TRAINS.1.to_string(), true),
    ];
    assert_eq!(chips(&run, &agent, cx), [(1, shown)]);
}

/// A reply without a chip hands nothing, whatever its words name, and no reply reaches a give
/// while tasks are off.
#[gpui::test]
fn a_reply_without_a_chip_hands_nothing(cx: &mut TestAppContext) {
    let run = run("handed-reply-words", cx);
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    run.finish(&agent, cx);
    let words = format!("Use tab {} at {} too", TRAINS.0, TRAINS.2);
    reply(&run, &agent, &words, &[], cx);
    assert_eq!(heard(&run, 1, 2), words);
    assert!(given(&run).is_empty());
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    assert!(chips(&run, &agent, cx).is_empty());
    assert!(!listed(&run, &agent, cx).contains(TRAINS.2));
    // Tasks off: nothing more is asked of a thread, with a chip or without.
    run.finish(&agent, cx);
    (run.agents).update(cx, |agents, cx| agents.set_tasks_on(false, cx));
    reply(&run, &agent, "And this", &[TRAINS.0], cx);
    assert!(given(&run).is_empty());
    assert_eq!(replies(&run, &agent, cx), [words]);
}

/// A reply that answers the thread's question hands its tabs as it does: the answer says so after
/// the user's words, and a tab alone answers it. With tasks off the words still answer, and no
/// tab goes.
#[gpui::test]
fn a_reply_that_answers_a_question_hands_its_tabs(cx: &mut TestAppContext) {
    let run = run("handed-answer", cx);
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    let ask = || json!({ "question": "Which train?", "answers": ["The 9:05", "The 10:05"] });
    let asked = call(&run.agents, &agent, "ask", ask(), cx);
    // No words and no tab that goes: it still waits.
    reply(&run, &agent, "", &[77], cx);
    assert!(asked.try_recv().is_err(), "nothing answered the question");
    reply(&run, &agent, "The 9:05", &[TRAINS.0], cx);
    assert_eq!(
        asked.try_recv().expect("answered").text,
        format!(
            "The user answered: The 9:05\n\nThe user added one of their tabs to your pages.\n{}",
            line_of(TRAINS)
        )
    );
    assert!(is_handed(&run, &agent, TRAINS.0, cx));
    let asked = call(&run.agents, &agent, "ask", ask(), cx);
    reply(&run, &agent, "", &[SEATS.0], cx);
    assert_eq!(
        asked.try_recv().expect("answered").text,
        format!(
            "The user added one of their tabs to your pages.\n{}",
            line_of(SEATS)
        )
    );
    assert_eq!(replies(&run, &agent, cx), ["The 9:05", ""]);
    // Tasks off: the answer lets it finish, and hands nothing.
    press_return(&run, &agent, SEATS.0, cx);
    let asked = call(&run.agents, &agent, "ask", ask(), cx);
    (run.agents).update(cx, |agents, cx| agents.set_tasks_on(false, cx));
    reply(&run, &agent, "The 10:05", &[SEATS.0], cx);
    assert_eq!(
        asked.try_recv().expect("answered").text,
        "The user answered: The 10:05"
    );
    assert_eq!(space_of(&run, SEATS.0), USERS);
    assert_eq!(given(&run), [TRAINS.0, SEATS.0]);
}

/// A new run of a thread from its own words says, at each message of the user's that handed it
/// tabs, how many and nothing of which, as they went back since; the reply it starts on says its
/// own in full.
#[gpui::test]
fn a_new_run_from_its_words_says_how_many_tabs_and_not_which(cx: &mut TestAppContext) {
    let run = run("handed-again", cx);
    // Nothing kept: a run that went starts again from the thread's words.
    (run.agents).update(cx, |agents, cx| agents.set_keeps_tasks(false, cx));
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    run.ran(1);
    run.finish(&agent, cx);
    assert_eq!(space_of(&run, TRAINS.0), USERS);
    run.said(&agent, Out::Gone(String::new()), cx);
    reply(&run, &agent, "And then?", &[SEATS.0], cx);
    let (_, first) = run.ran(2);
    assert_eq!(
        first,
        format!(
            "{ASKED}\n\nThe user added one of their tabs to your pages.\n\n\
             You: Two seats at 9:05, car 4.\n\n\
             Me: And then?\n\nThe user added one of their tabs to your pages.\n{}\n\n\
             Go on from here.",
            line_of(SEATS)
        )
    );
    assert!(
        !first.contains(TRAINS.2) && !first.contains(TRAINS.1),
        "{first}"
    );
}

/// A reply is recorded as its words, as it always was, and one with tabs says how many and
/// nothing of which.
#[test]
fn a_reply_is_recorded_as_its_words_and_how_many_tabs() {
    let plain = TaskAction::Reply("And for Sunday?".into());
    assert_eq!(format!("{plain:?}"), r#"Reply("And for Sunday?")"#);
    let with = TaskAction::Reply(handed::Reply {
        text: "And these".into(),
        tabs: Picked::of_chips(USERS, vec![TRAINS.0, SEATS.0]),
    });
    assert_eq!(format!("{with:?}"), r#"Reply("And these" with Picked(2))"#);
}

/// The replies that wait for `agent`'s turn under way to end: each one's words and tabs.
fn held(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<(String, Vec<u64>)> {
    run.agents.read_with(cx, |agents, _| {
        (agents.held_replies(agent).into_iter())
            .map(|reply| (reply.text.to_string(), reply.tabs.tabs().to_vec()))
            .collect()
    })
}

/// A thread at work is handed nothing mid-turn. A reply with a tab waits for the turn under way
/// to end, out of the conversation, and one sent after it waits behind it. At the turn's end each
/// is given in its order, its tabs checked again then: one that no longer fits isn't handed, and
/// a reply left with nothing to say isn't given. The thread stays under way on them.
#[gpui::test]
fn a_reply_with_a_tab_waits_for_the_turn_under_way_to_end(cx: &mut TestAppContext) {
    let run = run("handed-held", cx);
    (run.browser.tabs.borrow_mut()).push((FARES.0, USERS, FARES.1, FARES.2));
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    reply(&run, &agent, "And this", &[TRAINS.0], cx);
    reply(&run, &agent, "And then?", &[], cx);
    reply(&run, &agent, "", &[SEATS.0], cx);
    reply(&run, &agent, "Fares too", &[FARES.0], cx);
    let waits = [
        ("And this".to_string(), vec![TRAINS.0]),
        ("And then?".to_string(), vec![]),
        (String::new(), vec![SEATS.0]),
        ("Fares too".to_string(), vec![FARES.0]),
    ];
    assert_eq!(held(&run, &agent, cx), waits);
    assert!(given(&run).is_empty());
    assert!(replies(&run, &agent, cx).is_empty());
    assert!(entries(&run, &agent, handed::HANDED, cx).is_empty());
    for tab in [TRAINS.0, SEATS.0, FARES.0] {
        assert_eq!(space_of(&run, tab), USERS);
    }
    // Meanwhile one is no longer the core's to give, and one closed.
    run.browser.not_given.borrow_mut().insert(SEATS.0);
    (run.browser.tabs.borrow_mut()).retain(|tab| tab.0 != FARES.0);
    run.said(&agent, Out::Said("Two seats at 9:05, car 4.".into()), cx);
    run.said(&agent, Out::Ended(None), cx);
    cx.run_until_parked();
    assert_eq!(run.state(&agent, cx), State::Working);
    assert!(held(&run, &agent, cx).is_empty());
    assert_eq!(given(&run), [TRAINS.0]);
    assert_eq!(
        heard(&run, 1, 2),
        format!(
            "And this\n\nThe user added one of their tabs to your pages.\n{}",
            line_of(TRAINS)
        )
    );
    assert_eq!(heard(&run, 1, 3), "And then?");
    assert_eq!(heard(&run, 1, 4), "Fares too");
    assert_eq!(
        replies(&run, &agent, cx),
        ["And this", "And then?", "Fares too"]
    );
    // The chip of the tab that closed, by the title it had at the send.
    let shown = [
        (1, vec![(TRAINS.0, TRAINS.1.to_string(), true)]),
        (3, vec![(FARES.0, FARES.1.to_string(), false)]),
    ];
    assert_eq!(chips(&run, &agent, cx), shown);
    assert!(
        run.agents
            .update(cx, |agents, _| agents.take_unsent(&agent).is_empty())
    );
    // The turn they started ends as any does, with nothing held.
    run.finish(&agent, cx);
    assert_eq!(space_of(&run, TRAINS.0), USERS);
}

/// Stop means stop: a reply held when the thread is stopped, fails or finishes is never given.
/// Nothing is handed, no message joins the conversation, the log has no entry, and the user has
/// their words and picks back, once.
#[gpui::test]
fn a_reply_held_at_a_stop_is_never_given(cx: &mut TestAppContext) {
    use crate::agents::runner::Failure;
    type End = fn(&Run, &str, &mut TestAppContext);
    let ends: [(&str, End); 3] = [
        ("stop", |run, agent, cx| {
            run.agents.update(cx, |agents, cx| {
                agents.task_action(agent, TaskAction::Stop, cx);
            })
        }),
        ("failure", |run, agent, cx| {
            let failure = Failure::Other("It broke.".into());
            run.said(agent, Out::Ended(Some(failure)), cx)
        }),
        ("finish", |run, agent, cx| {
            let done = call(
                &run.agents,
                agent,
                "finish",
                json!({ "result": "Booked." }),
                cx,
            );
            cx.run_until_parked();
            assert!(!done.try_recv().expect("answered").error);
            // Its turn winds down after it.
            run.said(agent, Out::Ended(None), cx);
        }),
    ];
    for (name, end) in ends {
        let run = run(&format!("handed-held-{name}"), cx);
        let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
        run.ran(1);
        reply(&run, &agent, "And this", &[TRAINS.0], cx);
        reply(&run, &agent, "And then?", &[], cx);
        end(&run, &agent, cx);
        cx.run_until_parked();
        assert_ne!(run.state(&agent, cx), State::Working, "{name}");
        assert!(given(&run).is_empty(), "{name}");
        assert_eq!(space_of(&run, TRAINS.0), USERS, "{name}");
        assert!(replies(&run, &agent, cx).is_empty(), "{name}");
        assert!(chips(&run, &agent, cx).is_empty(), "{name}");
        assert!(held(&run, &agent, cx).is_empty(), "{name}");
        for action in [handed::HANDED, handed::GIVEN_BACK, handed::TAKEN_BACK] {
            assert!(entries(&run, &agent, action, cx).is_empty(), "{name}");
        }
        let unsent = |cx: &mut TestAppContext| -> Vec<(String, Vec<u64>)> {
            run.agents.update(cx, |agents, _| {
                (agents.take_unsent(&agent).into_iter())
                    .map(|reply| (reply.text.to_string(), reply.tabs.tabs().to_vec()))
                    .collect()
            })
        };
        let back = [
            ("And this".to_string(), vec![TRAINS.0]),
            ("And then?".to_string(), vec![]),
        ];
        assert_eq!(unsent(cx), back, "{name}");
        assert!(unsent(cx).is_empty(), "{name}: handed back twice");
        // The turn's end that follows gives nothing either.
        run.said(&agent, Out::Ended(None), cx);
        cx.run_until_parked();
        assert!(given(&run).is_empty(), "{name}");
        assert!(replies(&run, &agent, cx).is_empty(), "{name}");
    }
}

fn came_back(run: &Run, agent: &str, cx: &mut TestAppContext) -> Vec<u64> {
    run.agents
        .read_with(cx, |agents, cx| agents.came_back(agent, USERS, cx))
}

/// The tabs that went back as a thread stopped are what the user's next reply to it shows picked
/// again. Each follows its tab as it is now: one that left the site it came back on, closed, or
/// is no longer in the Space in view isn't picked. One the user took back themselves, or the
/// thread closed, never is. The reply sent, nothing is picked for the next.
#[gpui::test]
fn the_tabs_that_came_back_are_picked_for_the_next_reply(cx: &mut TestAppContext) {
    let run = run("handed-came-back", cx);
    (run.browser.tabs.borrow_mut()).push((FARES.0, USERS, FARES.1, FARES.2));
    (run.browser.tabs.borrow_mut()).push((15, USERS, "Maps", "https://maps.example/lisbon"));
    let picked = [TRAINS.0, SEATS.0, FARES.0, 15];
    let agent = send(&run, ASKED, USERS, &picked, cx).expect("started");
    run.ran(1);
    assert!(came_back(&run, &agent, cx).is_empty());
    // The user's own return and the thread's own close are no picks of the next reply.
    press_return(&run, &agent, 15, cx);
    run.finish(&agent, cx);
    assert_eq!(came_back(&run, &agent, cx), [TRAINS.0, SEATS.0, FARES.0]);
    // Seen from a Space they aren't in, none fits.
    let elsewhere = run
        .agents
        .read_with(cx, |agents, cx| agents.came_back(&agent, 2, cx));
    assert!(elsewhere.is_empty());
    // One left the site it came back on, and one closed.
    for tab in run.browser.tabs.borrow_mut().iter_mut() {
        if tab.0 == SEATS.0 {
            tab.3 = "https://other.example/seats";
        }
    }
    (run.browser.tabs.borrow_mut()).retain(|tab| tab.0 != FARES.0);
    assert_eq!(came_back(&run, &agent, cx), [TRAINS.0]);
    // Back on its site, it's picked again.
    for tab in run.browser.tabs.borrow_mut().iter_mut() {
        if tab.0 == SEATS.0 {
            tab.3 = "https://tickets.example/later";
        }
    }
    assert_eq!(came_back(&run, &agent, cx), [TRAINS.0, SEATS.0]);
    // The next reply carried what the user left picked: nothing is for the one after.
    reply(&run, &agent, "And then?", &[TRAINS.0], cx);
    assert!(came_back(&run, &agent, cx).is_empty());
    run.finish(&agent, cx);
    assert_eq!(came_back(&run, &agent, cx), [TRAINS.0]);
    // A thread put away comes back with them, and they go with the thread.
    (run.agents).update(cx, |agents, cx| agents.dismiss_task(&agent, cx));
    assert_eq!(came_back(&run, &agent, cx), [TRAINS.0]);
    (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx));
    assert!(came_back(&run, &agent, cx).is_empty());
}

/// Nothing ilo keeps of a thread has the title or the address of a tab the user handed it: not
/// the record History keeps of it, whose log has the tab's host alone, and no file of the
/// profile, the kept list among them.
#[gpui::test]
fn nothing_kept_has_a_handed_tabs_title_or_address(cx: &mut TestAppContext) {
    let run = run("handed-kept", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    run.ran(1);
    // The thread ends with it, is handed two, closes one, the user returns the other, and it
    // ends again.
    run.finish(&agent, cx);
    reply(&run, &agent, "And then?", &[TRAINS.0, SEATS.0], cx);
    press_return(&run, &agent, SEATS.0, cx);
    _ = answered(&run, &agent, "close_tab", TRAINS.0, cx);
    run.finish(&agent, cx);
    let ended = (run.agents).update(cx, |agents, cx| agents.take_ended(cx));
    let [(_, record)] = ended.as_slice() else {
        panic!("{} records of one thread", ended.len());
    };
    // Its record has each give and return, by the host.
    let noted = |action: &str| {
        let steps = record.steps.iter();
        let sites = steps.filter(|step| step.action == action);
        sites.map(|step| step.site.clone()).collect::<Vec<_>>()
    };
    assert_eq!(
        noted(handed::HANDED),
        ["example.com", "example.com", "tickets.example"]
    );
    assert_eq!(noted(handed::TAKEN_BACK), ["tickets.example"]);
    assert_eq!(noted(handed::GIVEN_BACK), ["example.com", "example.com"]);
    (run.agents).update(cx, |agents, cx| agents.end_tasks(true, cx));
    cx.run_until_parked();
    let files = run.files();
    assert!(
        (files.iter()).any(|(path, _)| path.ends_with("index.json")),
        "no kept list"
    );
    let kept = files.into_iter().map(|(path, text)| {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        (name, text)
    });
    for (name, text) in kept.chain([("its record".to_string(), format!("{record:?}"))]) {
        for of_the_tab in [TRAINS.1, SEATS.1, "/trains", "/seats"] {
            assert!(!text.contains(of_the_tab), "{name} keeps {of_the_tab}");
        }
    }
}

/// The composer offers a tab, and keeps its chip, only while the app would hand it: the user's
/// own tab on the web, in the Space in view, of the thread's profile and no private window's. It
/// is a read that leaves nothing; the core's own refusals are met at the give.
#[gpui::test]
fn the_composer_offers_only_a_tab_that_fits(cx: &mut TestAppContext) {
    let run = run("handed-fits", cx);
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
    let agent = send(&run, ASKED, USERS, &[], cx).expect("started");
    run.ran(1);
    let fits = |agent: Option<&str>, space: u64, tab: u64, cx: &mut TestAppContext| {
        run.agents
            .read_with(cx, |agents, cx| agents.still_fits(agent, tab, space, cx))
    };
    // A tab elsewhere, the outside agent's own, one not open, a private window's and one off the
    // web: none, for a thread to start or for this one.
    for (space, tab) in [
        (USERS, 13),
        (USERS, 20),
        (2, 20),
        (USERS, 99),
        (PRIVATE, 15),
        (USERS, 16),
    ] {
        assert!(!fits(None, space, tab, cx), "tab {tab} from {space}");
        assert!(
            !fits(Some(&agent), space, tab, cx),
            "tab {tab} from {space}"
        );
    }
    // Another profile's tab is one for a thread started there, and none for this one.
    assert!(fits(None, OTHER_PROFILE, 17, cx));
    assert!(!fits(Some(&agent), OTHER_PROFILE, 17, cx));
    // What only the core refuses is offered, and its chip says so at the send.
    for tab in [TRAINS.0, 18] {
        assert!(fits(None, USERS, tab, cx));
        assert!(fits(Some(&agent), USERS, tab, cx));
    }
    assert!(given(&run).is_empty());
    assert!(run.browser.reads.borrow().is_empty());
    // A tab a thread holds is in no Space of the user's, until it's back.
    run.finish(&agent, cx);
    reply(&run, &agent, "", &[TRAINS.0], cx);
    assert!(!fits(None, USERS, TRAINS.0, cx));
    assert!(!fits(Some(&agent), USERS, TRAINS.0, cx));
    press_return(&run, &agent, TRAINS.0, cx);
    assert!(fits(Some(&agent), USERS, TRAINS.0, cx));
    // Tasks off, none is offered.
    (run.agents).update(cx, |agents, cx| agents.set_tasks_on(false, cx));
    assert!(!fits(None, USERS, TRAINS.0, cx));
    assert!(!fits(Some(&agent), USERS, TRAINS.0, cx));
}

/// No tool gives a thread a tab of the user's, takes one back to them, or says anything of one
/// that isn't the thread's: every tool a thread and an outside agent has is called naming the
/// user's tab, and the core is asked for no give, the tab stays where it is, the log has no
/// return by the user, and no answer has its title or its site.
#[gpui::test]
fn no_tool_gives_returns_or_tells_of_a_tab_of_the_users(cx: &mut TestAppContext) {
    let run = run("handed-walk", cx);
    let agent = send(&run, ASKED, USERS, &[TRAINS.0], cx).expect("started");
    run.ran(1);
    let arguments = json!({
        "tab": SEATS.0,
        "tabs": [SEATS.0, TRAINS.0],
        "split": SEATS.0,
        "url": "https://own.example/page",
        "ref": "e1",
        "text": "x",
        "key": "Enter",
        "option": "x",
        "title": "Named",
        "reason": "To carry on.",
        "question": "Which?",
        "result": "Booked.",
        "method": "GET",
        "steps": [{ "tool": "click", "tab": SEATS.0, "ref": "e1" }],
    });
    for who in [agent.as_str(), "claude-code"] {
        let mut names = tools::names(who);
        assert!(names.len() > 10, "{who} has its tools");
        // What ends the thread goes last, so each other tool is called on one under way.
        names.sort_by_key(|name| matches!(name.as_str(), "finish" | "done"));
        for tool in names {
            let answer = call(&run.agents, who, &tool, arguments.clone(), cx);
            cx.run_until_parked();
            if let Ok(answer) = answer.try_recv() {
                for of_the_tab in [SEATS.1, "tickets.example", "/seats"] {
                    assert!(
                        !answer.text.contains(of_the_tab),
                        "{who}'s {tool} says {of_the_tab}"
                    );
                }
            }
            assert_eq!(given(&run), [TRAINS.0], "{who}'s {tool} gave a tab");
            assert_eq!(space_of(&run, SEATS.0), USERS, "{who}'s {tool}");
            assert!(!is_handed(&run, &agent, SEATS.0, cx), "{who}'s {tool}");
            for whose in [agent.as_str(), "claude-code"] {
                assert!(
                    entries(&run, whose, handed::TAKEN_BACK, cx).is_empty(),
                    "{who}'s {tool} returned a tab as the user"
                );
                assert_eq!(
                    entries(&run, whose, handed::HANDED, cx).len(),
                    usize::from(whose == agent),
                    "{who}'s {tool} entered a give"
                );
            }
        }
    }
}

/// A handed tab is one of its thread's pages under the thread's level, as any other: at Look
/// only, a click on it is refused and a read of it goes to the page, and the give's entry has
/// the thread's level.
#[gpui::test]
fn a_handed_tab_is_under_its_threads_level(cx: &mut TestAppContext) {
    use crate::agents::tier::Tier;
    let run = run("handed-look", cx);
    let start = Start {
        tier: Tier::Look,
        tabs: Picked::of_chips(USERS, vec![TRAINS.0]),
        ..Start::default()
    };
    let agent = send_with(&run, ASKED, start, cx).expect("started");
    run.ran(1);
    let click = answered(&run, &agent, "click", TRAINS.0, cx);
    assert!(click.error, "{}", click.text);
    let read = call(
        &run.agents,
        &agent,
        "read_page",
        json!({ "tab": TRAINS.0 }),
        cx,
    );
    cx.run_until_parked();
    assert!(read.try_recv().is_err(), "the read didn't reach the page");
    assert!(
        (run.browser.reads.borrow().iter()).any(|read| read.0 == TRAINS.0),
        "the read didn't reach the page"
    );
    let level = run.agents.read_with(cx, |agents, _| {
        let mut entries = agents.activity(&agent);
        let handed = entries.find(|entry| entry.action.as_ref() == handed::HANDED);
        handed.expect("its entry").tier.to_string()
    });
    assert_eq!(level, Tier::Look.label());
}
