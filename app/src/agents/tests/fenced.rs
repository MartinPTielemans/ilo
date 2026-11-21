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
    agents.update(cx, |agents, _| agents.tab_loaded(tab, url));
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

/// The agent's next call on `tab`, a snapshot, asks the user first with an open's question for
/// `site`, before the core is asked anything of the page.
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
