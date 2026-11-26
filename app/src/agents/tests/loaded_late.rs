//! An open or a navigate whose page was still loading as it stopped waiting, after the core's limit,
//! says it opened once that page loads after all, fine and at the site it was to open: the core
//! says so as the tab's load ends (tab_state_changed), as it does for every tab.

use super::*;
use crate::agents::task::{Line, TraceStep};

const LOADING: &str = "example.com is still loading after 15 s. Wait, or try again.";

/// Claude Code said hello on connection 1, as its server does before its first call.
fn hello(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    let event = Event::Hello {
        agent: "claude-code".into(),
        connection: 1,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(event, cx));
}

/// Calls `tool` on connection 1 as the server does, answers it from the core as still loading,
/// and tells the agents it was answered, as the server does once the answer has gone.
fn still_loading(
    agents: &Entity<Agents>,
    browser: &Made,
    id: u64,
    tool: &str,
    arguments: Value,
    cx: &mut TestAppContext,
) {
    let (reply, answer) = mpsc::channel();
    let call = Call {
        id,
        agent: "claude-code".into(),
        connection: 1,
        tool: tool.into(),
        arguments,
        reply,
        at: std::time::Instant::now(),
    };
    agents.update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.page_done(request, LOADING.into(), Some(Failure::Loading), cx)
    });
    let result = answer.recv().unwrap();
    assert_eq!(result.kind, Some(tools::Kind::StillLoading));
    let event = Event::Answered {
        agent: "claude-code".into(),
        id,
        at: std::time::Instant::now(),
        error: Some(result.text),
        kind: result.kind,
        image: None,
        notes: None,
    };
    agents.update(cx, |agents, cx| agents.handle(event, cx));
}

/// The core says `tab` is at `url`, loading or not, its page fine or not.
fn state(
    agents: &Entity<Agents>,
    tab: u64,
    url: &str,
    loading: bool,
    fine: bool,
    cx: &mut TestAppContext,
) {
    agents.update(cx, |agents, cx| {
        agents.tab_state(tab, url, loading, fine, cx)
    });
}

/// The steps of Claude Code's one session, as its conversation draws them: each one's title and
/// whether it failed.
fn steps(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<(String, bool)> {
    let talk = agents.read_with(cx, |agents, cx| {
        let cards = agents.shown_cards(cx);
        assert_eq!(cards.len(), 1, "one session");
        agents.conversation(&cards[0].agent, cx)
    });
    (talk.unwrap().lines.into_iter())
        .filter_map(|line| match line {
            Line::Step(TraceStep { title, failed, .. }) => Some((title.to_string(), failed)),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn an_open_still_loading_says_it_opened_once_its_page_loads(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    still_loading(
        &agents,
        &browser,
        1,
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    assert_eq!(
        steps(&agents, cx),
        [("Couldn't open example.com".into(), true)]
    );

    // Still loading, and then loaded: it opened.
    state(&agents, 30, "https://example.com/", true, true, cx);
    assert_eq!(
        steps(&agents, cx),
        [("Couldn't open example.com".into(), true)]
    );
    state(&agents, 30, "https://example.com/", false, true, cx);
    assert_eq!(steps(&agents, cx), [("Opened example.com".into(), false)]);
}

#[gpui::test]
fn a_navigate_still_loading_says_it_opened_once_its_page_loads(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let going = json!({ "tab": 20, "url": "iana.org" });
    still_loading(&agents, &browser, 1, "navigate", going, cx);
    state(&agents, 20, "https://www.iana.org/", false, true, cx);
    assert_eq!(steps(&agents, cx), [("Opened iana.org".into(), false)]);
}

/// A load that ends on an error page, or at another site, or after the tab closed, didn't open what
/// the step was to open: it still says it couldn't, and the conversation draws the steps that
/// couldn't as one. Only the load the call stopped waiting for counts.
#[gpui::test]
fn a_load_that_ends_badly_or_elsewhere_still_could_not_open(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let couldnt = |title: &str| (title.to_string(), true);

    still_loading(
        &agents,
        &browser,
        1,
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    state(&agents, 30, "https://example.com/", false, false, cx);
    // The next load is another one's.
    state(&agents, 30, "https://example.com/", false, true, cx);
    assert_eq!(steps(&agents, cx), [couldnt("Couldn't open example.com")]);

    still_loading(
        &agents,
        &browser,
        2,
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    state(&agents, 31, "https://www.example.org/", false, true, cx);
    assert_eq!(steps(&agents, cx), [couldnt("Couldn't open 2 pages")]);

    still_loading(
        &agents,
        &browser,
        3,
        "open_tab",
        json!({ "url": "example.com" }),
        cx,
    );
    agents.update(cx, |agents, cx| agents.tab_closed(32, cx));
    state(&agents, 32, "https://example.com/", false, true, cx);
    assert_eq!(steps(&agents, cx), [couldnt("Couldn't open 3 pages")]);
}
