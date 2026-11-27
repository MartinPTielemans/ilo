//! A task the user stopped: the last line of its log says so, with the pages it held then. An
//! outside agent's pages are refused to its client from then, on every connection and at every
//! level, until the user hands them back on that line.

use super::*;
use crate::agents::task::ilo::{self, IloButton, IloKind, IloLine};

/// ilo's lines of the task `key`'s conversation that say the user stopped it.
pub(super) fn you_stopped(
    agents: &Entity<Agents>,
    key: &str,
    cx: &mut TestAppContext,
) -> Vec<IloLine> {
    (conversation(agents, key, cx).lines.into_iter())
        .filter_map(|line| match line {
            Line::Ilo(line) if matches!(line.kind, IloKind::YouStopped { .. }) => Some(line),
            _ => None,
        })
        .collect()
}

/// The user's Stop on Claude Code's task `key`.
pub(super) fn stop(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| {
        agents.task_action(key, TaskAction::Stop, cx)
    });
    assert!(card(agents, key, cx).status.starts_with("Stopped"));
}

/// The user's Stop is the last line of an outside agent's session's log: that they stopped it,
/// with how many pages it held then, said once, and with one thing to press, Hand Back.
#[gpui::test]
fn the_users_stop_is_the_last_line_of_a_sessions_log(cx: &mut TestAppContext) {
    let (agents, _, first) = lowered(Tier::Work, cx);
    assert!(you_stopped(&agents, &first, cx).is_empty());
    stop(&agents, &first, cx);

    let talk = conversation(&agents, &first, cx);
    let Some(Line::Ilo(last)) = talk.lines.last() else {
        panic!("its log ends with {:?}", talk.lines.last());
    };
    assert_eq!(last.kind, IloKind::YouStopped { pages: 1 });
    assert_eq!(ilo::buttons(last), [IloButton::PagesBack]);
    assert_eq!(you_stopped(&agents, &first, cx).len(), 1);
}

/// A session that held no page when the user stopped it has the line with no count.
#[gpui::test]
fn a_stopped_session_with_no_page_counts_none(cx: &mut TestAppContext) {
    let (agents, browser) = at_work(cx);
    let first = cards(&agents, cx)[0].agent.to_string();
    // Its page left its Space, as one the user moved to their own does.
    browser.tabs.borrow_mut().retain(|tab| tab.0 != 20);
    stop(&agents, &first, cx);
    let lines = you_stopped(&agents, &first, cx);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].kind, IloKind::YouStopped { pages: 0 });
}

/// `answer` was refused at once, in the one text a stopped task's calls get.
#[track_caller]
fn refused_as_stopped(answer: mpsc::Receiver<ToolResult>) {
    let refused = answer.try_recv().expect("the call wasn't answered at once");
    assert!(refused.error, "it went: {}", refused.text);
    assert_eq!(refused.text, STOPPED);
}

/// The log's refusals of Claude Code's calls as stopped, newest first, each by its action.
fn refusals(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<String> {
    agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.refused == STOPPED)
            .map(|entry| entry.action.to_string())
            .collect()
    })
}

/// The log's entries of a stopped task's pages handed back, each by its site and its count.
fn handed_back(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<(String, String)> {
    agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.action == tools::PAGES_BACK)
            .map(|entry| (entry.site.to_string(), entry.target.to_string()))
            .collect()
    })
}

/// The user's press of Hand Back on the stopped line of the task `key`.
fn hand_back(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| {
        agents.task_action(key, TaskAction::PagesBack, cx)
    });
}

/// As if the stopped line of the task `key` had stood past its guard.
fn stood(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) {
    agents.update(cx, |agents, _| {
        let session = (agents.task_mut(key)).and_then(|task| task.session.as_mut());
        let stopped = session.and_then(|session| session.stopped.as_mut());
        *stopped.expect("a session the user stopped") -= tier::PROTECT;
    });
}

/// The pages of the task `key`'s session.
fn rail(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> Vec<u64> {
    agents.read_with(cx, |agents, _| {
        let session = agents.task(key).and_then(|task| task.session.as_ref());
        session.expect("its session").tabs.clone()
    })
}

/// After the user's Stop, the task's page is refused to its client on the connection it worked
/// on and on a new one, with the task at Full access, whatever the tool: each in the one text,
/// which holds neither the page's title nor its site. Nothing goes to the page, no task starts,
/// the stopped task's log gets no line, and the activity log has each refusal.
#[gpui::test]
fn a_stopped_tasks_page_is_refused_on_every_connection_whatever_the_tool(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Full, cx);
    (browser.tabs.borrow_mut()).push((31, 2, "Other", "https://example.org/"));
    stop(&agents, &first, cx);
    let (title, address) = {
        let tabs = browser.tabs.borrow();
        let page = tabs.iter().find(|tab| tab.0 == 20).expect("its page");
        (page.2, page.3)
    };
    let host = site(address);
    assert!(!title.is_empty() && !host.is_empty());
    assert!(!STOPPED.contains(title) && !STOPPED.contains(&host));
    let talk = conversation(&agents, &first, cx).lines.len();
    let (pages, tabs) = (browser.pages.borrow().len(), browser.tabs.borrow().len());
    let now = std::time::Instant::now;

    // Where it worked: a call that names the page, and one that names none and so is on it.
    refused_as_stopped(coupon(&agents, (1, 4, 20), cx));
    refused_as_stopped(call_on(&agents, 1, 5, "snapshot", json!({}), now(), cx));

    hello_on(&agents, "claude-code", 2, "claude-code", cx);
    let calls = [
        ("click", json!({ "tab": 20, "ref": "e1" })),
        ("type", json!({ "tab": 20, "ref": "e1", "text": "SAVE10" })),
        ("select", json!({ "tab": 20, "ref": "e1", "option": "One" })),
        ("press_key", json!({ "tab": 20, "key": "Enter" })),
        ("scroll", json!({ "tab": 20 })),
        ("read_page", json!({ "tab": 20 })),
        ("snapshot", json!({ "tab": 20 })),
        ("screenshot", json!({ "tab": 20 })),
        ("navigate", json!({ "tab": 20, "url": "example.org" })),
        (
            "request",
            json!({ "tab": 20, "url": "https://example.com/api", "method": "GET" }),
        ),
        (
            "run_steps",
            json!({ "tab": 20, "steps": [{ "action": "click", "ref": "e1" }] }),
        ),
        (
            "upload",
            json!({ "tab": 20, "ref": "e1", "path": "/tmp/a" }),
        ),
        ("close_tab", json!({ "tab": 20 })),
        ("hand_off", json!({ "tab": 20, "reason": "Sign in" })),
        ("ask_for_tab", json!({ "tab": 20, "reason": "To go on" })),
        ("pin_tab", json!({ "tab": 20 })),
        ("list_split", json!({ "tab": 20 })),
        ("separate_tab", json!({ "tab": 20 })),
        ("split_tabs", json!({ "tabs": [31, 20] })),
        ("add_to_split", json!({ "tab": 31, "split": 20 })),
    ];
    let count = calls.len();
    for (nth, (tool, arguments)) in calls.into_iter().enumerate() {
        refused_as_stopped(call_on(
            &agents,
            2,
            10 + nth as u64,
            tool,
            arguments,
            now(),
            cx,
        ));
    }
    // Under every tool, the one check of a page says the same.
    let floor = agents.update(cx, |agents, cx| {
        agents.may_act("claude-code", 20, "click", cx)
    });
    assert_eq!(floor.expect_err("its page went").why, STOPPED);

    let shown = cards(&agents, cx);
    assert_eq!(shown.len(), 1, "a refused call started a task");
    assert_eq!(shown[0].agent, first);
    assert_eq!(conversation(&agents, &first, cx).lines.len(), talk);
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "a call went to the page"
    );
    assert_eq!(browser.tabs.borrow().len(), tabs);
    assert_eq!(rail(&agents, &first, cx), [20]);
    let logged = refusals(&agents, cx);
    assert_eq!(logged.len(), count + 2, "{logged:?}");
    assert_eq!(logged[0], "add_to_split");
}

/// A call that came before the Stop and isn't answered yet is the stopped task's, whatever it
/// would open: an open_tab that waited its turn opens nothing and starts no task (#4209). It's
/// told by when it came, not by its connection: the next call there opens its page, a new task's.
#[gpui::test]
fn an_open_that_came_before_the_stop_opens_nothing_and_starts_no_task(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Work, cx);
    let came = std::time::Instant::now();
    stop(&agents, &first, cx);
    let tabs = browser.tabs.borrow().len();
    let open = json!({ "url": "example.org" });
    refused_as_stopped(call_on(&agents, 1, 4, "open_tab", open.clone(), came, cx));
    assert_eq!(
        cards(&agents, cx).len(),
        1,
        "the call that waited started a task"
    );
    assert_eq!(
        browser.tabs.borrow().len(),
        tabs,
        "the call that waited opened a page"
    );
    assert_eq!(refusals(&agents, cx), ["open_tab"]);

    let later = std::time::Instant::now();
    let answer = call_on(&agents, 1, 5, "open_tab", open, later, cx);
    let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
    let second = cards(&agents, cx)[0].agent.to_string();
    assert_ne!(first, second);
    assert_eq!(rail(&agents, &second, cx), [30]);
}

/// The refusal's entry has a site only from a page of that client's own sessions: a call that came
/// before the Stop and names a tab of the user's leaves an entry with none, where one on the
/// stopped task's own page has its site, as a refusal's entry does.
#[gpui::test]
fn a_refused_calls_entry_has_no_site_from_a_tab_that_isnt_its_clients(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Work, cx);
    (browser.tabs.borrow_mut()).push((41, 1, "Bank", "https://bank.example/"));
    let came = std::time::Instant::now();
    stop(&agents, &first, cx);
    let sites = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            (agents.activity("claude-code"))
                .filter(|entry| entry.refused == STOPPED)
                .map(|entry| entry.site.to_string())
                .collect::<Vec<_>>()
        })
    };
    let read = |id: u64, tab: u64, cx: &mut TestAppContext| {
        let arguments = json!({ "tab": tab });
        refused_as_stopped(call_on(&agents, 1, id, "read_page", arguments, came, cx));
    };
    read(4, 41, cx);
    assert_eq!(sites(cx), [String::new()]);
    read(5, 20, cx);
    let own = site(
        browser
            .tabs
            .borrow()
            .iter()
            .find(|tab| tab.0 == 20)
            .unwrap()
            .3,
    );
    assert_eq!(sites(cx), [own, String::new()]);
}

/// A stopped task that is dropped, as the Archive keeps only so many, closes its pages: the
/// withholding goes with the task, and its client reaches none of the pages it held.
#[gpui::test]
fn a_dropped_stopped_tasks_pages_close(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Work, cx);
    stop(&agents, &first, cx);
    agents.update(cx, |agents, cx| {
        assert!(agents.withheld("claude-code", 20));
        agents.drop_task(&first, cx);
        assert!(!agents.withheld("claude-code", 20));
    });
    assert!(
        !browser.tabs.borrow().iter().any(|tab| tab.0 == 20),
        "its page stayed open"
    );

    hello_on(&agents, "claude-code", 2, "claude-code", cx);
    let now = std::time::Instant::now;
    let pages = browser.pages.borrow().len();
    let clicked = coupon(&agents, (2, 4, 20), cx).try_recv();
    let clicked = clicked.expect("the click wasn't answered at once");
    assert!(clicked.error, "it clicked: {}", clicked.text);
    answered(&agents, 4, cx);
    assert_eq!(browser.pages.borrow().len(), pages, "the click went");
    let listed = call_on(&agents, 2, 5, "list_tabs", json!({}), now(), cx);
    let listed = listed.try_recv().expect("its tabs, at once");
    assert_eq!(listed.text, tools::tab_list(&[], None, &[]));
}

/// On a new connection the stopped task's page isn't listed, and a call that names no tab never
/// falls on it. A page that connection opens is a new task's, at the level the stopped one was
/// left at, and that task holds none of the stopped task's pages.
#[gpui::test]
fn a_new_task_has_none_of_the_stopped_tasks_pages(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Look, cx);
    stop(&agents, &first, cx);
    hello_on(&agents, "claude-code", 2, "claude-code", cx);
    let now = std::time::Instant::now;
    let listed = |id: u64, cx: &mut TestAppContext| {
        let answer = call_on(&agents, 2, id, "list_tabs", json!({}), now(), cx);
        let listed = answer.try_recv().expect("its tabs, at once");
        answered(&agents, id, cx);
        listed.text
    };
    assert_eq!(listed(4, cx), tools::tab_list(&[], None, &[]));
    assert_eq!(cards(&agents, cx).len(), 1);

    let pages = browser.pages.borrow().len();
    let fell = call_on(&agents, 2, 5, "snapshot", json!({}), now(), cx);
    let fell = fell.try_recv().expect("an answer at once");
    assert_eq!(fell.text, NO_TAB);
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "it fell on the stopped page"
    );
    answered(&agents, 5, cx);

    let answer = call_on(
        &agents,
        2,
        6,
        "open_tab",
        json!({ "url": "example.org" }),
        now(),
        cx,
    );
    let opened = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(opened.text.starts_with("Opened tab 30."), "{}", opened.text);
    answered(&agents, 6, cx);
    let second = (cards(&agents, cx).into_iter())
        .map(|card| card.agent.to_string())
        .find(|key| *key != first)
        .expect("the new connection's task");
    assert_eq!(tier(&agents, &second, cx), Tier::Look);
    assert_eq!(rail(&agents, &second, cx), [30]);
    assert_eq!(rail(&agents, &first, cx), [20]);
    let tabs = listed(7, cx);
    let ids: Vec<&str> = (tabs.lines())
        .filter_map(|line| line.trim_start_matches(['*', ' ']).split(' ').next())
        .collect();
    assert_eq!(ids, ["30"], "{tabs}");
}

/// What the user does with a stopped task's page gives nothing back: taking it over and handing
/// that tab back, as a click in it and its strip do, or opening it in their own Space. Only the
/// stopped line's own Hand Back does.
#[gpui::test]
fn the_users_own_click_and_open_give_nothing_back(cx: &mut TestAppContext) {
    let (agents, _, first) = lowered(Tier::Work, cx);
    stop(&agents, &first, cx);
    hello_on(&agents, "claude-code", 2, "claude-code", cx);

    agents.update(cx, |agents, cx| agents.take_over("claude-code", 20, cx));
    refused_as_stopped(coupon(&agents, (2, 4, 20), cx));
    agents.update(cx, |agents, cx| agents.hand_back(20, cx));
    refused_as_stopped(coupon(&agents, (2, 5, 20), cx));
    agents.update(cx, |agents, cx| {
        agents.task_action(&first, TaskAction::OpenTab(20), cx)
    });
    refused_as_stopped(coupon(&agents, (2, 6, 20), cx));

    let lines = you_stopped(&agents, &first, cx);
    assert_eq!(ilo::buttons(&lines[0]), [IloButton::PagesBack]);
    assert!(handed_back(&agents, cx).is_empty());
    assert_eq!(cards(&agents, cx).len(), 1);
}

/// Hand Back on the stopped line is the way back, and the user's press alone: one that comes as
/// the line does is ignored. Past that, the pages are the client's again, the line keeps its
/// words and loses its button, and the log has one entry, with a count and no site. A task of
/// that client's then takes the page at no more than the stopped task's level, and the user's
/// Stop of that task withholds it again.
#[gpui::test]
fn hand_back_gives_a_stopped_tasks_pages_back_at_its_level(cx: &mut TestAppContext) {
    let (agents, browser, first) = lowered(Tier::Look, cx);
    stop(&agents, &first, cx);
    hello_on(&agents, "claude-code", 2, "claude-code", cx);

    hand_back(&agents, &first, cx);
    refused_as_stopped(coupon(&agents, (2, 4, 20), cx));
    assert!(
        handed_back(&agents, cx).is_empty(),
        "a press as the line came"
    );

    stood(&agents, &first, cx);
    hand_back(&agents, &first, cx);
    let lines = you_stopped(&agents, &first, cx);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].kind, IloKind::YouStopped { pages: 1 });
    assert!(
        ilo::buttons(&lines[0]).is_empty(),
        "the line kept its button"
    );
    hand_back(&agents, &first, cx);
    assert_eq!(
        handed_back(&agents, cx),
        [(String::new(), "1 page".to_string())]
    );

    // Taken by the new connection's task, which is held to Look only from then.
    let pages = browser.pages.borrow().len();
    refused_as_look_only(coupon(&agents, (2, 5, 20), cx));
    answered(&agents, 5, cx);
    assert_eq!(browser.pages.borrow().len(), pages, "the click went");
    let second = (cards(&agents, cx).into_iter())
        .map(|card| card.agent.to_string())
        .find(|key| *key != first)
        .expect("the new connection's task");
    assert_eq!(tier(&agents, &second, cx), Tier::Look);
    assert_eq!(rail(&agents, &second, cx), [20]);

    // Stop again is the way out again.
    stop(&agents, &second, cx);
    hello_on(&agents, "claude-code", 3, "claude-code", cx);
    refused_as_stopped(coupon(&agents, (3, 6, 20), cx));
}

/// The entry of the hand back says how many pages, as the stopped line does, or none (Z32).
#[test]
fn the_hand_backs_entry_counts_its_pages() {
    let title = |pages: usize| {
        let did = tools::Did {
            tool: tools::PAGES_BACK.into(),
            target: tools::pages_counted(pages).unwrap_or_default(),
            ..Default::default()
        };
        tools::step_title(&did, tools::Tense::Done)
    };
    assert_eq!(title(0), "You handed it back");
    assert_eq!(title(1), "You handed it back · 1 page");
    assert_eq!(title(3), "You handed it back · 3 pages");
}

/// A call that names no tool of the agent's, in the words an entry of the browser's own or of the
/// user's has, is answered as no tool and leaves nothing that could read as theirs: no entry in
/// the activity log, which Settings › Agents and a past thread list, and no step in the task's
/// trace. So while the task works on its page, and once the user stopped it, when every call on
/// that page has an entry by its tool's name. The name the activity log gave the assistant's own
/// web search, before threads searched with the browser's, is one.
#[gpui::test]
fn a_call_by_an_entrys_name_is_no_tool_and_leaves_nothing(cx: &mut TestAppContext) {
    const NAMES: [&str; 7] = [
        "request_permission camera",
        "request_permission midi",
        "file_dialog file_input",
        "file_dialog file_system_access",
        "request_device usb",
        "taken back",
        "web_search",
    ];
    let (agents, browser, first) = lowered(Tier::Full, cx);
    let now = std::time::Instant::now;
    let no_tool = |name: &str, id: u64, arguments: Value, cx: &mut TestAppContext| {
        let answer = call_on(&agents, 1, id, name, arguments, now(), cx);
        let answer = answer.try_recv().expect("the call wasn't answered at once");
        assert_eq!(
            answer,
            ToolResult::error(format!("There's no tool {name}."))
        );
    };
    let left = |cx: &mut TestAppContext| {
        let kept = log(&agents, "claude-code", cx);
        let steps: Vec<String> = agents.read_with(cx, |agents, _| {
            let trace = &agents.task(&first).expect("its task").trace;
            trace.iter().map(|step| step.did.tool.clone()).collect()
        });
        (kept, steps, browser.pages.borrow().len())
    };
    let before = left(cx);
    for (at, name) in NAMES.into_iter().enumerate() {
        no_tool(name, 10 + at as u64, json!({ "tab": 20 }), cx);
    }
    assert_eq!(left(cx), before, "a call by an entry's name left something");

    stop(&agents, &first, cx);
    let before = left(cx);
    for (at, name) in NAMES.into_iter().enumerate() {
        no_tool(name, 20 + 2 * at as u64, json!({ "tab": 20 }), cx);
        no_tool(name, 21 + 2 * at as u64, json!({}), cx);
    }
    assert_eq!(left(cx), before, "a call by an entry's name left something");

    // Nothing the log holds reads as a permission the browser refused a page.
    for (action, site, _) in &before.0 {
        assert!(!NAMES.contains(&action.as_str()), "{action}");
        assert_eq!(tools::permission_stopped(action, site), None);
    }
    for tool in &before.1 {
        assert!(!NAMES.contains(&tool.as_str()), "{tool}");
    }
}
