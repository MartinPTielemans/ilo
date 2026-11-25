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
