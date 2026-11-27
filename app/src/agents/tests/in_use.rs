//! A page that is using the camera or the microphone is only read below Full access: a tool that
//! isn't on the list such a page answers is refused there at once, whatever page of the thread's
//! it is, and one made later is refused until it's placed.

use super::*;
use crate::agents::access::IN_USE_ANSWERS;
use crate::agents::tier::IN_USE_TOLD;

/// The tools and steps a page in use refuses below Full access: each navigates the page or
/// changes what it does.
const REFUSED: [&str; 6] = ["navigate", "click", "type", "select", "press_key", "upload"];

/// A thread's own tools a page in use refuses below Full access, as it does any tool not placed:
/// a search given the tab takes the page to its results, and a strand works in the tab it's given.
const THREADS_REFUSED: [&str; 2] = [tools::SEARCHED, tools::STRAND];

/// What a call of `tool` on tab 20 says.
fn said(tool: &str) -> Value {
    match tool {
        "navigate" => json!({ "tab": 20, "url": "https://www.example.com/next" }),
        "press_key" => json!({ "tab": 20, "key": "Enter" }),
        "type" => json!({ "tab": 20, "ref": "e1", "text": "SAVE10" }),
        "select" => json!({ "tab": 20, "ref": "e1", "option": "One" }),
        _ => json!({ "tab": 20, "ref": "e1" }),
    }
}

/// Claude Code with a session under way on tab 20, whose page is in use.
fn on_a_page_in_use(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    browser.in_use.borrow_mut().insert(20);
    (agents, browser)
}

/// What the core was asked to do in a page, or where to take one, so far.
fn asked(browser: &Made) -> (usize, usize) {
    let (pages, navigated) = (browser.pages.borrow(), browser.navigated.borrow());
    (pages.len(), navigated.len())
}

/// Every tool an agent from outside or a thread is offered, and every step a run takes, is on
/// exactly one list, answered or refused, and no list names anything else: a new one fails here
/// until it's placed, and is refused on a page in use until then.
#[test]
fn every_tool_and_step_is_answered_or_refused_on_a_page_in_use() {
    let mut every: Vec<String> = ["claude-code", "task-1"]
        .into_iter()
        .flat_map(tools::names)
        .collect();
    every.extend(tools::STEPS.map(String::from));
    for name in &every {
        let on = [&IN_USE_ANSWERS[..], &REFUSED[..], &THREADS_REFUSED[..]]
            .iter()
            .filter(|list| list.contains(&name.as_str()))
            .count();
        assert_eq!(on, 1, "{name} is on {on} of the lists");
    }
    for name in (IN_USE_ANSWERS.iter().chain(&REFUSED)).chain(&THREADS_REFUSED) {
        assert!(every.iter().any(|each| each == name), "{name} is no tool");
    }
}

/// Below Full access each tool that isn't a read is refused at once on a page in use: the
/// assistant hears the one text, the log's entry has the user's line by the page's host alone,
/// the thread shows the refusal with Full access as the level that lifts it, and the core is
/// asked nothing. A call that names no tab is on that page too.
#[gpui::test]
fn a_page_in_use_refuses_all_but_reads_below_full_access(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    for tier in [Tier::Look, Tier::Work] {
        give(&agents, tier, cx);
        let line = tier::why_line(Why::InUse, tier, "example.com");
        for tool in REFUSED {
            for named in [true, false] {
                let before = asked(&browser);
                let mut said = said(tool);
                if !named {
                    said.as_object_mut().unwrap().remove("tab");
                }
                let answer = call(&agents, "claude-code", tool, said, cx);
                let refused = (answer.try_recv())
                    .unwrap_or_else(|_| panic!("{tool} wasn't refused at once at {tier:?}"));
                assert!(refused.error, "{tool}");
                assert_eq!(refused.text, IN_USE_TOLD, "{tool}");
                assert_eq!(asked(&browser), before, "the core was asked for {tool}");
                let (_, site, why) = log(&agents, "claude-code", cx).remove(0);
                assert_eq!(
                    (site.as_str(), why.as_str()),
                    ("example.com", line.as_str())
                );
                let stop = stops(&agents, cx).pop().expect("the refusal shows");
                assert_eq!((stop.why, stop.tier, stop.tab), (Why::InUse, tier, 20));
                assert_eq!(stop.tier.lifting(stop.why), Some(Tier::Full));
            }
        }
    }
}

/// A read is answered on a page in use at every level, as on any page, and so is each tool that
/// leaves the page as it is.
#[gpui::test]
fn a_page_in_use_answers_its_reads_at_every_level(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    for tier in Tier::ALL {
        give(&agents, tier, cx);
        for tool in ["snapshot", "scroll", "screenshot"] {
            let before = asked(&browser).0;
            let answer = call(&agents, "claude-code", tool, json!({ "tab": 20 }), cx);
            assert_eq!(asked(&browser).0, before + 1, "{tool} at {tier:?}");
            assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
        }
        for tool in ["list_tabs", "list_split", "read_page", "title", "pin_tab"] {
            let said = json!({ "tab": 20, "title": "Basket" });
            if let Ok(answer) = call(&agents, "claude-code", tool, said, cx).try_recv() {
                assert_ne!(answer.text, IN_USE_TOLD, "{tool} at {tier:?}");
            }
        }
        let stops = stops(&agents, cx);
        assert_eq!(stops.iter().find(|stop| stop.why == Why::InUse), None);
    }
}

/// At Full access nothing is refused for the page being in use, and on a page that isn't in use
/// nothing changes at any level.
#[gpui::test]
fn full_access_and_a_page_not_in_use_are_as_ever(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    let goes = |cx: &mut TestAppContext| {
        let before = asked(&browser);
        for tool in REFUSED {
            if let Ok(answer) = call(&agents, "claude-code", tool, said(tool), cx).try_recv() {
                assert_ne!(answer.text, IN_USE_TOLD, "{tool}");
            }
        }
        // The core was asked for the navigate and for the key, which need no ref of the page.
        assert_ne!(asked(&browser), before);
        let stops = stops(&agents, cx);
        assert_eq!(stops.iter().find(|stop| stop.why == Why::InUse), None);
    };
    give(&agents, Tier::Full, cx);
    goes(cx);
    browser.in_use.borrow_mut().clear();
    give(&agents, Tier::Work, cx);
    goes(cx);
}

/// A run is judged step by step on a page in use: its reads are taken, and it stops at the first
/// step that isn't one, with the one text and what it had done.
#[gpui::test]
fn a_run_on_a_page_in_use_stops_at_its_first_step_that_isnt_a_read(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    let steps = json!({ "tab": 20, "steps": [
        { "action": "scroll" },
        { "action": "type", "ref": "e1", "text": "SAVE10" },
        { "action": "scroll" },
    ] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    assert_eq!(
        browser.pages.borrow().last().unwrap().2,
        PageAction::ScrollDown
    );
    let before = asked(&browser);
    shop_done(&agents, &browser, cx);
    let stopped = run.recv().unwrap();
    assert!(stopped.error);
    let told = format!("Step 2 failed: {IN_USE_TOLD}");
    assert!(stopped.text.starts_with(&told), "{}", stopped.text);
    assert!(
        stopped.text.ends_with("1. Scrolled down."),
        "{}",
        stopped.text
    );
    assert_eq!(
        asked(&browser),
        before,
        "the run took a step past its reads"
    );
}

/// A site the user set to Never for the agent keeps its own line, which no level lifts, on a page
/// in use too.
#[gpui::test]
fn a_never_keeps_its_line_on_a_page_in_use(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    agents.update(cx, |agents, _| {
        let answers = agents.answers_of("claude-code");
        (agents.grants.entry(answers).or_default()).insert("example.com".into(), false);
    });
    let before = asked(&browser);
    let answer = call(&agents, "claude-code", "navigate", said("navigate"), cx);
    let refused = answer.try_recv().expect("refused at once");
    assert!(
        refused.error && refused.text.contains("Never"),
        "{}",
        refused.text
    );
    assert_eq!(asked(&browser), before);
    let stop = stops(&agents, cx).pop().expect("the refusal shows");
    assert_eq!(stop.why, Why::Never);
    assert_eq!(stop.tier.lifting(stop.why), None);
}

/// A tab that isn't the agent's, and one the user holds, are refused as they are: the call hears
/// nothing of what the page is doing, and no refusal of this kind shows.
#[gpui::test]
fn a_tab_not_the_agents_or_held_by_the_user_says_nothing_of_its_page(cx: &mut TestAppContext) {
    let (agents, browser) = on_a_page_in_use(cx);
    browser.in_use.borrow_mut().insert(10);
    let theirs = json!({ "tab": 10, "key": "Enter" });
    let answer = call(&agents, "claude-code", "press_key", theirs, cx);
    assert_ne!(answer.try_recv().expect("refused").text, IN_USE_TOLD);
    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    for tool in REFUSED {
        if let Ok(answer) = call(&agents, "claude-code", tool, said(tool), cx).try_recv() {
            assert_ne!(answer.text, IN_USE_TOLD, "{tool}");
        }
    }
    let stops = stops(&agents, cx);
    assert_eq!(stops.iter().find(|stop| stop.why == Why::InUse), None);
}
