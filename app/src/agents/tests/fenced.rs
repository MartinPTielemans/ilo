//! Sites the user fenced (#3724), as an agent comes to one: by an open the user answers, or any
//! other way, which asks before the agent's next call on the tab, a read included.

use super::*;
use crate::agents::fence;

/// Claude Code, connected, with mybank.example fenced.
fn fenced(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        assert_eq!(
            agents
                .fence("https://www.MyBank.example/login", cx)
                .as_deref(),
            Some("mybank.example")
        );
    });
    (agents, browser)
}

/// Tab `tab`'s page goes to `url` without the agent opening it, as a click, a redirect, back or
/// the page itself takes it, and the core says it loaded.
fn arrives(agents: &Entity<Agents>, browser: &Made, tab: u64, url: &str, cx: &mut TestAppContext) {
    let url: &'static str = Box::leak(url.to_string().into_boxed_str());
    for each in (browser.tabs.borrow_mut().iter_mut()).filter(|each| each.0 == tab) {
        each.2 = "MyBank";
        each.3 = url;
    }
    agents.update(cx, |agents, _| agents.tab_at(tab, url));
}

/// A page with a button that goes somewhere.
fn more_tree() -> String {
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Shop" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "button" }, "name": { "value": "Show more" },
          "backendDOMNodeId": 71 },
    ]})
    .to_string()
}

/// The bank's page, whose words never reach an agent the user hasn't let stay.
fn bank_tree() -> String {
    json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "MyBank" },
          "childIds": ["2", "3"] },
        { "nodeId": "2", "role": { "value": "heading" }, "name": { "value": "Balance 1,234" } },
        { "nodeId": "3", "role": { "value": "button" }, "name": { "value": "Show more" },
          "backendDOMNodeId": 81 },
    ]})
    .to_string()
}

/// The answer to a call made as the tab came to `site`: it says only where the tab is.
fn withheld(answer: &ToolResult, tab: u64, site: &str) {
    assert!(!answer.error, "{}", answer.text);
    assert!(
        answer.text.ends_with(&fence::withheld(tab, site)),
        "{}",
        answer.text
    );
    assert!(!answer.text.contains("Balance"), "{}", answer.text);
}

/// The agent's next call on `tab`, a snapshot, asks the user first to use `site`, with an open's
/// answers, before the core is asked anything of the page.
fn asks_next(
    agents: &Entity<Agents>,
    browser: &Made,
    tab: u64,
    site: &str,
    cx: &mut TestAppContext,
) -> (mpsc::Receiver<ToolResult>, Question) {
    let (pages, reads) = (browser.pages.borrow().len(), browser.reads.borrow().len());
    let answer = call(agents, "claude-code", "snapshot", json!({ "tab": tab }), cx);
    let question = (browser.asked.borrow().clone())
        .unwrap_or_else(|| panic!("the call on tab {tab} at {site} didn't ask"));
    assert_eq!(question.what, fence::arrival(site));
    assert_eq!(question.why, fence::why(site));
    assert!(question.fenced && !question.each_time);
    assert_eq!(browser.pages.borrow().len(), pages, "the page was asked of");
    assert_eq!(browser.reads.borrow().len(), reads, "the page was read");
    assert!(answer.try_recv().is_err(), "it answered before the user");
    (answer, question)
}

/// Each way a tab comes to a fenced site but an open the user allowed, a click on another site, a
/// redirect after an open of another, back, the page itself, a tab a page opened and a tab the
/// user handed over, asks before the agent's next call there, and what the call that took it there
/// answers says nothing of the page, not even its outline (#3724).
#[gpui::test]
fn each_way_onto_a_fenced_site_asks_before_the_next_call(cx: &mut TestAppContext) {
    // A click on another site.
    let (agents, browser) = fenced(cx);
    let answer = click_on(&agents, &browser, more_tree(), "Show more", cx);
    assert_eq!(*browser.asked.borrow(), None, "a click off the fence asked");
    arrives(&agents, &browser, 20, "https://login.mybank.example/", cx);
    let clicked = page_done(&agents, &browser, answer, bank_tree(), cx);
    withheld(&clicked, 20, "login.mybank.example");
    asks_next(&agents, &browser, 20, "login.mybank.example", cx);

    // A redirect after an open of another site, and back.
    for url in ["shop.example", "back"] {
        let (agents, browser) = fenced(cx);
        let go = json!({ "tab": 20, "url": url });
        let answer = call(&agents, "claude-code", "navigate", go, cx);
        assert_eq!(*browser.asked.borrow(), None, "{url} asked");
        arrives(&agents, &browser, 20, "https://mybank.example/signin", cx);
        withheld(
            &page_done(&agents, &browser, answer, bank_tree(), cx),
            20,
            "mybank.example",
        );
        asks_next(&agents, &browser, 20, "mybank.example", cx);
    }

    // The page itself.
    let (agents, browser) = fenced(cx);
    arrives(&agents, &browser, 20, "https://www.mybank.example/", cx);
    asks_next(&agents, &browser, 20, "mybank.example", cx);

    // A tab the agent's page opened.
    let (agents, browser) = fenced(cx);
    (browser.tabs.borrow_mut()).push((26, 2, "MyBank", "https://app.mybank.example/"));
    agents.update(cx, |agents, cx| agents.tab_opened(26, 20, cx));
    asks_next(&agents, &browser, 26, "app.mybank.example", cx);

    // A tab the user handed over, moved to the agent's Space: handing it isn't an open's yes.
    let (agents, browser) = fenced(cx);
    (browser.tabs.borrow_mut()).push((25, 2, "MyBank", "https://mybank.example/"));
    asks_next(&agents, &browser, 25, "mybank.example", cx);
}

/// Every call that reads or acts on the page asks, a read included, and leaving the tab, closing
/// it or listing tabs doesn't; list_tabs gives it no title or address (#3724).
#[gpui::test]
fn a_tab_on_a_fenced_site_it_came_to_gives_nothing_unasked(cx: &mut TestAppContext) {
    for (tool, arguments) in [
        ("read_page", json!({ "tab": 20 })),
        ("screenshot", json!({ "tab": 20 })),
        ("click", json!({ "tab": 20, "ref": "e1" })),
        ("type", json!({ "tab": 20, "ref": "e1", "text": "x" })),
        ("press_key", json!({ "tab": 20, "key": "Enter" })),
        ("scroll", json!({ "tab": 20, "direction": "down" })),
        (
            "request",
            json!({ "tab": 20, "requests": [{ "method": "GET", "path": "/" }] }),
        ),
        (
            "run_steps",
            json!({ "tab": 20, "steps": [{ "action": "scroll", "direction": "down" }] }),
        ),
    ] {
        let (agents, browser) = fenced(cx);
        arrives(&agents, &browser, 20, "https://mybank.example/", cx);
        let answer = call(&agents, "claude-code", tool, arguments, cx);
        let question = browser.asked.borrow().clone();
        let question = question.unwrap_or_else(|| panic!("{tool} didn't ask"));
        assert_eq!(question.what, fence::arrival("mybank.example"), "{tool}");
        assert!(browser.pages.borrow().is_empty(), "{tool} asked the page");
        assert!(browser.reads.borrow().is_empty(), "{tool} read the page");
        assert!(
            browser.requests.borrow().is_empty(),
            "{tool} sent a request"
        );
        assert!(answer.try_recv().is_err(), "{tool} answered");
    }

    let (agents, browser) = fenced(cx);
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    let listed = listed.recv().unwrap();
    assert_eq!(*browser.asked.borrow(), None, "list_tabs asked");
    assert!(listed.text.contains(fence::LISTED), "{}", listed.text);
    assert!(!listed.text.contains("mybank"), "{}", listed.text);
    assert!(!listed.text.contains("MyBank"), "{}", listed.text);
    let away = json!({ "tab": 20, "url": "https://news.example/" });
    let _left = call(&agents, "claude-code", "navigate", away, cx);
    assert_eq!(*browser.asked.borrow(), None, "leaving asked");
    assert_eq!(
        *browser.navigated.borrow(),
        [(20, "https://news.example/".to_string())]
    );
}

/// Allow Once lets the tab stay, its call going as it came, until the tab leaves the fenced site,
/// and coming back asks again; the log has the question and the answer (#3724).
#[gpui::test]
fn allow_once_lets_the_tab_stay_until_it_leaves(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let (answer, question) = asks_next(&agents, &browser, 20, "mybank.example", cx);
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Always, cx)
    });
    let snapshot = page_done(&agents, &browser, answer, bank_tree(), cx);
    assert!(snapshot.text.contains("Balance 1,234"), "{}", snapshot.text);
    let grants = agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert!(grants.is_empty(), "the fence's answer kept {grants:?}");
    let entries = log(&agents, "claude-code", cx);
    for action in [fence::ASKED_ACTION, fence::CAME_ACTION, "snapshot"] {
        let entry = (
            action.to_string(),
            "mybank.example".to_string(),
            String::new(),
        );
        assert!(entries.contains(&entry), "no {action} in {entries:?}");
    }

    // A page further in, under the same entry, stays allowed.
    arrives(
        &agents,
        &browser,
        20,
        "https://app.mybank.example/accounts",
        cx,
    );
    let again = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(
        *browser.asked.borrow(),
        None,
        "the tab let stay asked again"
    );
    assert!(!page_done(&agents, &browser, again, bank_tree(), cx).error);

    arrives(&agents, &browser, 20, "https://news.example/", cx);
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    asks_next(&agents, &browser, 20, "mybank.example", cx);
}

/// Never at the fence refuses as an open's Never does: the site is refused for the agent, unasked,
/// from then on, and the refusal says nothing of the list (#3724).
#[gpui::test]
fn never_at_the_fence_refuses_as_an_opens_never(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let (answer, question) = asks_next(&agents, &browser, 20, "mybank.example", cx);
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Never, cx)
    });
    let refused = answer.recv().unwrap();
    assert!(refused.error);
    assert!(!refused.text.contains("first"), "{}", refused.text);
    let grants = agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert_eq!(grants, [("mybank.example".into(), false)]);

    let read = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    assert_eq!(*browser.asked.borrow(), None, "a site answered Never asked");
    assert!(read.try_recv().expect("it wasn't refused").error);
    assert!(browser.reads.borrow().is_empty(), "the page was read");
}

/// An open of a fenced site asks every time, an Always Allow given there before included, its
/// card keeping no answer for good; once allowed the tab stays, and taking the site off the list
/// ends the fence at once (#3724).
#[gpui::test]
fn an_open_of_a_fenced_site_asks_every_time(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    let go = json!({ "tab": 20, "url": "mybank.example" });
    let answer = call(&agents, "claude-code", "navigate", go.clone(), cx);
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    assert_eq!(question.why, fence::why("mybank.example"));
    assert!(question.fenced);
    assert!(
        browser.navigated.borrow().is_empty(),
        "it went before the answer"
    );
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Always, cx)
    });
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let opened = page_done(&agents, &browser, answer, bank_tree(), cx);
    assert!(!opened.error && !opened.text.contains(&fence::withheld(20, "mybank.example")));
    let read = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(*browser.asked.borrow(), None, "the tab it opened asked");
    assert!(!page_done(&agents, &browser, read, bank_tree(), cx).error);

    let _again = call(&agents, "claude-code", "navigate", go, cx);
    assert!(
        browser.asked.borrow().is_some(),
        "the second open didn't ask"
    );
    let question = browser.asked.borrow().clone().unwrap();
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });

    let tab = json!({ "url": "https://app.mybank.example/" });
    let opening = call(&agents, "claude-code", "open_tab", tab, cx);
    let question = browser.asked.borrow().clone().expect("open_tab didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(!page_done(&agents, &browser, opening, bank_tree(), cx).error);
    let read = call(&agents, "claude-code", "snapshot", json!({ "tab": 30 }), cx);
    assert_eq!(
        *browser.asked.borrow(),
        None,
        "the tab open_tab opened asked"
    );
    assert!(!page_done(&agents, &browser, read, bank_tree(), cx).error);
    let grants = agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert!(grants.is_empty(), "an open kept {grants:?}");

    agents.update(cx, |agents, cx| agents.unfence("mybank.example", cx));
    (browser.tabs.borrow_mut()).push((25, 2, "MyBank", "https://mybank.example/"));
    let read = call(&agents, "claude-code", "snapshot", json!({ "tab": 25 }), cx);
    assert_eq!(*browser.asked.borrow(), None, "a site off the list asked");
    assert!(!page_done(&agents, &browser, read, bank_tree(), cx).error);
}

/// On a fenced site the tab may stay on, each click, key or form the agent sends asks every time,
/// whatever it answered on the site before, a payment's question staying a payment's; its
/// requests ask too, reads included (#3724).
#[gpui::test]
fn each_thing_sent_on_a_fenced_site_asks(cx: &mut TestAppContext) {
    // Not a bank's, whose money words would ask as payments.
    let (agents, browser, _) = connected(cx);
    // Allowed for good before the site was fenced.
    arrives(&agents, &browser, 20, "https://pension.example/", cx);
    let answer = click_on(&agents, &browser, post_tree(), "Send", cx);
    let question = browser.asked.borrow().clone().expect("the Send didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Always, cx)
    });
    assert!(!page_done(&agents, &browser, answer, post_tree(), cx).error);
    let grants = agents.read_with(cx, |agents, _| agents.grants("claude-code"));
    assert_eq!(grants, [("pension.example".into(), true)]);
    agents.update(cx, |agents, cx| _ = agents.fence("pension.example", cx));
    let (answer, question) = asks_next(&agents, &browser, 20, "pension.example", cx);
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    page_done(&agents, &browser, answer, post_tree(), cx);

    for name in ["Send", "Send"] {
        let answer = click_on(&agents, &browser, post_tree(), name, cx);
        let question = (browser.asked.borrow().clone()).expect("a click on the fence didn't ask");
        assert_eq!(question.why, fence::why("pension.example"));
        assert!(question.fenced);
        agents.update(cx, |agents, cx| {
            agents.answer(question.id, Answer::Once, cx)
        });
        assert!(!page_done(&agents, &browser, answer, post_tree(), cx).error);
    }
    let answer = click_on(&agents, &browser, shop_tree(), "Place order", cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the payment didn't ask");
    assert_eq!(question.why, "It looks like a payment or an order.");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(!page_done(&agents, &browser, answer, shop_tree(), cx).error);
    let scroll = json!({ "tab": 20, "direction": "down" });
    let answer = call(&agents, "claude-code", "scroll", scroll, cx);
    assert_eq!(*browser.asked.borrow(), None, "a scroll asked");
    assert!(!page_done(&agents, &browser, answer, post_tree(), cx).error);

    let get = json!({ "tab": 20, "requests": [{ "method": "GET", "path": "/api/balance" }] });
    let answer = call(&agents, "claude-code", "request", get, cx);
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the read request didn't ask");
    assert_eq!(question.what, fence::requests(1, "pension.example"));
    assert_eq!(question.why, fence::why("pension.example"));
    assert!(question.each_time && !question.fenced);
    assert!(browser.requests.borrow().is_empty(), "it was sent unasked");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert_eq!(
        browser.requests.borrow().len(),
        1,
        "the allowed request wasn't sent"
    );
    drop(answer);
}

/// A run whose step takes the tab to a fenced site stops there, saying nothing of the page
/// (#3724).
#[gpui::test]
fn a_run_stops_where_it_comes_to_a_fenced_site(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    let steps = json!({ "tab": 20, "steps": [
        { "action": "scroll", "direction": "down" },
        { "action": "scroll", "direction": "down" },
    ] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let pages = browser.pages.borrow().len();
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let ran = page_done(&agents, &browser, run, bank_tree(), cx);
    assert!(
        ran.text
            .contains(&fence::run_stopped(2, 20, "mybank.example")),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("Balance"), "{}", ran.text);
    assert_eq!(browser.pages.borrow().len(), pages, "the run went on");
}

/// The list is saved with the agents' settings, each host once, and comes back as it was (#3724).
#[gpui::test]
fn the_fenced_sites_are_kept(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("agents-test-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Agents");
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.clone()), cx);
    agents.update(cx, |agents, cx| {
        for typed in [
            "mybank.example",
            "MYBANK.example.",
            "not a site",
            "localhost:3000",
        ] {
            _ = agents.fence(typed, cx);
        }
    });
    let saved = std::fs::read_to_string(&path).unwrap();
    let later = self::agents(&browser, &configs, None, cx);
    later.update(cx, |agents, cx| agents.load(&saved, cx));
    let kept = later.read_with(cx, |agents, _| agents.fenced().to_vec());
    assert_eq!(kept, ["mybank.example", "localhost"]);
    later.update(cx, |agents, cx| agents.unfence("mybank.example", cx));
    let kept = later.read_with(cx, |agents, _| agents.fenced().to_vec());
    assert_eq!(kept, ["localhost"]);
    std::fs::remove_dir_all(dir).ok();
}

/// A wait or a check whose tab a page takes to a fenced site while it looks ends at once with that
/// alone: the same bytes whether the text it looks for is on the page or not, and whether the page
/// is busy or settled, so no try tells the agent anything of the page; and it isn't read for the
/// agent again (#3724).
#[gpui::test]
fn a_wait_on_a_tab_that_came_to_a_fenced_site_says_the_same_whatever_the_page(
    cx: &mut TestAppContext,
) {
    let busy = |tree: String| {
        let mut tree: Value = serde_json::from_str(&tree).unwrap();
        tree["busy"] = json!(true);
        tree.to_string()
    };
    let mut answers = Vec::new();
    for (step, tree) in [
        (
            json!({ "action": "wait", "text": "Balance" }),
            Some(bank_tree()),
        ),
        (
            json!({ "action": "wait", "text": "Balance" }),
            Some(more_tree()),
        ),
        (
            json!({ "action": "check", "text": "Balance" }),
            Some(bank_tree()),
        ),
        (
            json!({ "action": "check", "text": "Balance" }),
            Some(more_tree()),
        ),
        (
            json!({ "action": "wait", "gone": "Balance" }),
            Some(bank_tree()),
        ),
        (
            json!({ "action": "wait", "gone": "Balance" }),
            Some(more_tree()),
        ),
        (
            json!({ "action": "wait", "until": "quiet" }),
            Some(bank_tree()),
        ),
        (
            json!({ "action": "wait", "until": "quiet" }),
            Some(busy(bank_tree())),
        ),
        // A wait of time alone looks once its time is up: by then the page is the fence's.
        (json!({ "action": "wait", "seconds": 1 }), None),
    ] {
        let (agents, browser) = fenced(cx);
        let steps = json!({ "tab": 20, "steps": [step.clone()] });
        let run = call(&agents, "claude-code", "run_steps", steps, cx);
        arrives(&agents, &browser, 20, "https://mybank.example/", cx);
        if let Some(tree) = tree {
            let request = browser.pages.borrow().last().unwrap().1;
            agents.update(cx, |agents, cx| agents.page_done(request, tree, false, cx));
        }
        let looks = browser.pages.borrow().len();
        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();
        assert_eq!(
            browser.pages.borrow().len(),
            looks,
            "{step}: it read the page again"
        );
        answers.push((step, run.recv().unwrap()));
    }
    let said = format!("Ran 1 steps.\n1. {}", fence::withheld(20, "mybank.example"));
    for (step, answer) in answers {
        assert_eq!(answer, ToolResult::text(said.clone()), "{step}");
    }
}

/// What an answer about a tab that came to a fenced site may say: its number, and the site in the
/// withheld line alone, with no title, address or words of its page, nor which dialog it shows.
fn says_nothing_of_the_page(what: &str, answer: &ToolResult) {
    let text = answer
        .text
        .replace(&fence::withheld(20, "mybank.example"), "");
    for told in ["mybank", "MyBank", "Balance", "balance", "alert", "dialog"] {
        assert!(
            !text.contains(told),
            "{what} told {told:?}: {}",
            answer.text
        );
    }
}

/// Every other answer about a tab that came to a fenced site, the split views', pins', a close, a
/// handoff, asking for it back and sorting by site, says nothing of its page; sorting places it as
/// a tab of no site, not beside its site's others; and so with its page showing a dialog (#3724).
#[gpui::test]
fn every_other_answer_about_a_tab_that_came_to_a_fenced_site_is_without_its_page(
    cx: &mut TestAppContext,
) {
    let tab = |tab| json!({ "tab": tab });
    let reason = json!({ "tab": 20, "reason": "Check the balance" });
    let calls: [&[(&str, Value)]; 11] = [
        &[("list_split", tab(20))],
        &[("split_tabs", json!({ "tabs": [20, 21] }))],
        &[("add_to_split", json!({ "tab": 20, "split": 21 }))],
        &[
            ("split_tabs", json!({ "tabs": [20, 21] })),
            ("separate_tab", tab(20)),
        ],
        &[("pin_tab", tab(20))],
        &[("pin_tab", tab(20)), ("unpin_tab", tab(20))],
        &[("pin_tab", tab(20)), ("remove_tab", tab(20))],
        &[("close_tab", tab(20))],
        &[("ask_for_tab", reason.clone())],
        &[("sort_tabs_by_site", json!({}))],
        &[("hand_off", reason)],
    ];
    for (dialog, calls) in [false, true]
        .into_iter()
        .flat_map(|dialog| calls.map(|calls| (dialog, calls)))
    {
        let (agents, browser) = fenced(cx);
        (browser.tabs.borrow_mut()).extend([
            (21, 2, "Docs", "https://docs.example.com/"),
            (22, 2, "Docs", "https://docs.example.com/b"),
        ]);
        browser.splits.borrow_mut().push(vec![21, 22]);
        arrives(&agents, &browser, 20, "https://mybank.example/account", cx);
        if dialog {
            agents.update(cx, |agents, cx| {
                agents.page_dialog(20, Some(DialogKind::Alert), "Your balance is 1,234", cx)
            });
        }
        for (tool, arguments) in calls {
            let what = format!("{tool} (dialog {dialog})");
            let answer = call(&agents, "claude-code", tool, arguments.clone(), cx);
            // A handoff's card is its own, not the fence's.
            let asked = browser.asked.borrow().clone();
            assert!(
                asked.is_none_or(|asked| asked.handoff.is_some()),
                "{what} asked"
            );
            let answer = match *tool {
                "hand_off" if !dialog => {
                    assert!(
                        answer.try_recv().is_err(),
                        "{what} answered before the user"
                    );
                    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
                    answer.recv().unwrap()
                }
                _ => answer.recv().unwrap(),
            };
            says_nothing_of_the_page(&what, &answer);
            if *tool == "close_tab" && dialog {
                assert!(
                    answer.text.contains(fence::STAYS_OPEN),
                    "{what}: {}",
                    answer.text
                );
            }
            if *tool == "hand_off" && !dialog {
                // The user had the tab: handing it back isn't an open's yes either.
                asks_next(&agents, &browser, 20, "mybank.example", cx);
            }
        }
    }

    // Sorting by site: the tab of the site the agent opened stays where it is, not beside it.
    let (agents, browser) = fenced(cx);
    (browser.tabs.borrow_mut()).push((21, 2, "Docs", "https://docs.example.com/"));
    let opening = call(
        &agents,
        "claude-code",
        "open_tab",
        json!({ "url": "https://app.mybank.example/" }),
        cx,
    );
    let question = browser.asked.borrow().clone().expect("open_tab didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(!page_done(&agents, &browser, opening, bank_tree(), cx).error);
    arrives(&agents, &browser, 20, "https://mybank.example/account", cx);
    let sorted = call(&agents, "claude-code", "sort_tabs_by_site", json!({}), cx);
    assert_eq!(
        sorted.recv().unwrap(),
        ToolResult::text("Your Today tabs are together by site already.")
    );
    let order: Vec<u64> = (browser.tabs.borrow().iter())
        .filter(|tab| tab.1 == 2)
        .map(|tab| tab.0)
        .collect();
    assert_eq!(order, [20, 21, 30]);
}

/// A download from a fenced site the tab came to answers the agent's call without its name or how
/// it ends, while the user is asked as for any download; one the user allowed opening keeps its
/// name (#3724).
#[gpui::test]
fn a_download_from_a_fenced_site_gives_the_agent_no_name(cx: &mut TestAppContext) {
    let download = |request, id| Download {
        agent: "claude-code".into(),
        tab: 20,
        request,
        id,
        name: "statement-1234.pdf".into(),
        url: "https://mybank.example/statement-1234.pdf".into(),
        tab_empty: false,
    };
    let (agents, browser) = fenced(cx);
    let answer = click_on(&agents, &browser, more_tree(), "Show more", cx);
    let request = browser.pages.borrow().last().unwrap().1;
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    agents.update(cx, |agents, cx| {
        agents.download_started(download(request, 900), cx)
    });
    // At once, while the user is asked: it waits on nothing of theirs.
    let answer = answer
        .try_recv()
        .expect("the download wasn't answered at once");
    assert_eq!(answer, ToolResult::text(fence::download("mybank.example")));
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the download didn't ask");
    assert_eq!(
        question.what,
        "download statement-1234.pdf from mybank.example"
    );
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    let done = DownloadEnd::Done {
        name: "statement-1234.pdf".into(),
        bytes: 48_213,
        path: "/Users/someone/Downloads/statement-1234.pdf".into(),
    };
    agents.update(cx, |agents, cx| agents.download_ended(900, done, cx));

    // An open of it the user allowed is their yes.
    let (agents, browser) = fenced(cx);
    let go = json!({ "tab": 20, "url": "https://mybank.example/statement-1234.pdf" });
    let answer = call(&agents, "claude-code", "navigate", go, cx);
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    let request = browser.pages.borrow().last().unwrap().1;
    agents.update(cx, |agents, cx| {
        agents.download_started(download(request, 901), cx)
    });
    let question = browser
        .asked
        .borrow()
        .clone()
        .expect("the download didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    let done = DownloadEnd::Done {
        name: "statement-1234.pdf".into(),
        bytes: 48_213,
        path: "/Users/someone/Downloads/statement-1234.pdf".into(),
    };
    agents.update(cx, |agents, cx| agents.download_ended(901, done, cx));
    assert!(
        answer
            .recv()
            .unwrap()
            .text
            .starts_with("Downloaded statement-1234.pdf")
    );
    // The open didn't take the tab there, so a later way there asks.
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    asks_next(&agents, &browser, 20, "mybank.example", cx);
}

/// A navigate the user allowed that fails takes their yes back, so the tab coming there later by a
/// link asks; a tab let stay on the site before keeps that (#3724).
#[gpui::test]
fn a_failed_open_takes_its_yes_back(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    let go = |url: &str| json!({ "tab": 20, "url": url });
    *browser.cant_go.borrow_mut() = true;
    let answer = call(&agents, "claude-code", "navigate", go("mybank.example"), cx);
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert_eq!(
        answer.recv().unwrap(),
        ToolResult::error("The tab couldn't go there.")
    );
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    let (answer, question) = asks_next(&agents, &browser, 20, "mybank.example", cx);

    // Let stay, then an open further in that fails: the tab stays let stay where it is.
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    page_done(&agents, &browser, answer, bank_tree(), cx);
    let answer = call(
        &agents,
        "claude-code",
        "navigate",
        go("app.mybank.example"),
        cx,
    );
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    assert!(answer.recv().unwrap().error);
    let read = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert_eq!(*browser.asked.borrow(), None, "the tab let stay asked");
    assert!(!page_done(&agents, &browser, read, bank_tree(), cx).error);
}

/// The yes ends once the tab is off the entry by any address the core reports, going back or
/// forward included, a page the back-forward cache restores being one that loads nothing; and an
/// allowed open that went elsewhere, by a redirect, ends it too (#3724).
#[gpui::test]
fn back_and_forward_onto_a_fenced_site_ask_again(cx: &mut TestAppContext) {
    let (agents, browser) = fenced(cx);
    let go = json!({ "tab": 20, "url": "mybank.example" });
    let answer = call(&agents, "claude-code", "navigate", go.clone(), cx);
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    assert!(!page_done(&agents, &browser, answer, bank_tree(), cx).error);

    // Back to where it was, and forward again, each restored with no load.
    for (way, url) in [
        ("back", "https://www.example.com/"),
        ("forward", "https://mybank.example/"),
    ] {
        let moved = call(
            &agents,
            "claude-code",
            "navigate",
            json!({ "tab": 20, "url": way }),
            cx,
        );
        assert_eq!(*browser.asked.borrow(), None, "{way} asked");
        arrives(&agents, &browser, 20, url, cx);
        let moved = page_done(&agents, &browser, moved, bank_tree(), cx);
        if way == "forward" {
            withheld(&moved, 20, "mybank.example");
        }
    }
    asks_next(&agents, &browser, 20, "mybank.example", cx);

    // An allowed open whose page redirected off the site, then a link back onto it.
    let (agents, browser) = fenced(cx);
    let answer = call(&agents, "claude-code", "navigate", go, cx);
    let question = browser.asked.borrow().clone().expect("the open didn't ask");
    agents.update(cx, |agents, cx| {
        agents.answer(question.id, Answer::Once, cx)
    });
    arrives(&agents, &browser, 20, "https://other.example/", cx);
    page_done(&agents, &browser, answer, more_tree(), cx);
    arrives(&agents, &browser, 20, "https://mybank.example/", cx);
    asks_next(&agents, &browser, 20, "mybank.example", cx);
}
