//! page_log (#4377): what a tab's page reported, which the core keeps for the agent's own tabs. The
//! app passes the core's JSON on and adds nothing but one sentence for a page not in Developer
//! Mode, which only the user turns on.

use super::fenced::{arrives, fenced};
use super::*;
use crate::agents::fence;
use crate::agents::tools::{
    CONSOLE, Did, NEEDS_DEVELOPER_MODE, PAGE_LOG_FAILED, Tense, step_title,
};

/// What the core holds for a page not in Developer Mode: its failed requests, trimmed.
const TRIMMED: &str = r#"{"developer_mode":false,"requests":[{"method":"GET","url":"https://www.example.com/missing","status":404}],"dropped":0}"#;
/// And for one in it: the console and the requests whole.
const WHOLE: &str = r#"{"developer_mode":true,"console":[{"level":"error","text":"Boom","line":3}],"requests":[{"method":"GET","url":"https://www.example.com/missing?q=1","status":404}],"dropped":0}"#;

/// Calls page_log on tab 20, whose page the core reports as `log` says, in Developer Mode or not.
fn page_log(
    agents: &Entity<Agents>,
    browser: &Made,
    log: &str,
    developer_mode: bool,
    cx: &mut TestAppContext,
) -> ToolResult {
    (browser.page_logs.borrow_mut()).insert(20, (log.into(), developer_mode));
    let answer = call(agents, "claude-code", "page_log", json!({ "tab": 20 }), cx);
    answer.try_recv().expect("page_log answers at once")
}

/// The answer is the core's JSON as it came, with the one sentence about Developer Mode after it
/// for a page that isn't in it, and nothing added for one that is. The same at every level.
#[gpui::test]
fn the_answer_is_the_cores_log_and_for_a_page_not_in_developer_mode_one_sentence(
    cx: &mut TestAppContext,
) {
    let (agents, browser, _) = connected(cx);
    let started = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, started, shop_tree(), cx);
    for tier in Tier::ALL {
        give(&agents, tier, cx);
        let trimmed = page_log(&agents, &browser, TRIMMED, false, cx);
        assert!(!trimmed.error, "{tier:?}: {}", trimmed.text);
        assert_eq!(
            trimmed.text,
            format!("{TRIMMED}\n\n{NEEDS_DEVELOPER_MODE}"),
            "{tier:?}"
        );
        let whole = page_log(&agents, &browser, WHOLE, true, cx);
        assert!(!whole.error, "{tier:?}: {}", whole.text);
        assert_eq!(whole.text, WHOLE, "{tier:?}");
    }
}

/// A tab the agent doesn't own is refused as read_page's is, and the core is never asked for its
/// log, so none of another tab's reaches the agent.
#[gpui::test]
fn a_tab_not_the_agents_is_refused_and_the_core_is_not_asked_for_its_log(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    (browser.page_logs.borrow_mut()).insert(10, (WHOLE.into(), true));
    let answer = call(&agents, "claude-code", "page_log", json!({ "tab": 10 }), cx);
    let answer = answer.recv().unwrap();
    assert_eq!(
        answer,
        ToolResult::error("The tab isn't in the agent's own Space.")
    );
    assert_eq!(*browser.page_log_asked.borrow(), []);
    let (action, site, why) = log(&agents, "claude-code", cx).remove(0);
    assert_eq!(
        (action.as_str(), site.as_str(), why.as_str()),
        (
            PAGE_LOG_FAILED,
            "en.wikipedia.org",
            "The tab isn't in the agent's own Space."
        )
    );
}

/// A tab on a site the user put off limits is refused, whatever the level, and the core isn't
/// asked for its log.
#[gpui::test]
fn a_tab_on_an_off_limits_site_is_refused_and_the_core_is_not_asked_for_its_log(
    cx: &mut TestAppContext,
) {
    let (agents, browser) = fenced(cx);
    arrives(&agents, &browser, 20, "https://www.mybank.example/", cx);
    (browser.page_logs.borrow_mut()).insert(20, (WHOLE.into(), true));
    let answer = call(&agents, "claude-code", "page_log", json!({ "tab": 20 }), cx);
    let site = "mybank.example";
    let refused = refused(
        &agents,
        &browser,
        answer,
        &fence::arrival(site),
        Why::Fenced,
        site,
        cx,
    );
    assert!(!refused.text.contains("Boom"), "{}", refused.text);
    assert_eq!(*browser.page_log_asked.borrow(), []);
}

/// The log's entry is one a call, naming the host, and says the console was read only for a page
/// in Developer Mode, and failed requests elsewhere.
#[gpui::test]
fn each_call_is_one_entry_naming_the_host_and_what_it_read(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    page_log(&agents, &browser, TRIMMED, false, cx);
    page_log(&agents, &browser, WHOLE, true, cx);
    // Newest first, after the connection's own entry.
    let entries: Vec<_> = log(&agents, "claude-code", cx)
        .into_iter()
        .take(2)
        .collect();
    assert_eq!(
        entries,
        [
            (CONSOLE.into(), "example.com".into(), String::new()),
            (PAGE_LOG_FAILED.into(), "example.com".into(), String::new()),
        ]
    );
    let step = |tool: &str, tense| {
        let did = Did {
            tool: tool.into(),
            site: "example.com".into(),
            ..Default::default()
        };
        step_title(&did, tense)
    };
    assert_eq!(
        step(CONSOLE, Tense::Now),
        "Reading the console on example.com"
    );
    assert_eq!(
        step(CONSOLE, Tense::Done),
        "Read the console on example.com"
    );
    assert_eq!(
        step(PAGE_LOG_FAILED, Tense::Now),
        "Reading failed requests on example.com"
    );
    assert_eq!(
        step(PAGE_LOG_FAILED, Tense::Done),
        "Read failed requests on example.com"
    );
}

/// An outside agent and a thread are both offered page_log, and it says Developer Mode is the
/// user's.
#[test]
fn both_kinds_of_agent_are_offered_page_log() {
    for agent in ["claude-code", "task-1"] {
        let list = tools::list(agent);
        let tool = (list.as_array().unwrap().iter())
            .find(|tool| tool["name"] == "page_log")
            .unwrap_or_else(|| panic!("{agent} has no page_log"));
        assert_eq!(
            tool["description"],
            "What the page reported since it loaded: failed requests, and in Developer Mode the console and full requests. Only the user can turn Developer Mode on, in the site's menu."
        );
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
}

/// Developer Mode and its list of sites are the user's: no tool, of an outside agent's or a
/// thread's, turns it on or off, or lists the sites it's on for.
#[test]
fn no_tool_reaches_developer_mode_or_its_list() {
    for agent in ["claude-code", "task-1"] {
        for tool in tools::list(agent).as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            let properties = tool["inputSchema"]["properties"].as_object();
            let named: Vec<&String> = properties.map_or(Vec::new(), |each| each.keys().collect());
            for word in ["developer", "devtools"] {
                assert!(!name.contains(word), "{agent}'s tool {name} names {word}");
                assert!(
                    named.iter().all(|each| !each.contains(word)),
                    "{agent}'s {name} takes {named:?}"
                );
            }
        }
        for wanted in ["developer_mode", "developer_mode_sites", "devtools"] {
            assert!(!tools::has(agent, wanted), "{agent} has {wanted}");
        }
    }
}

/// A log longer than an answer loses whole entries, oldest first, and no more than it must: the
/// JSON left still parses, the count line says how many went, and PL3 comes last and whole.
#[test]
fn a_log_too_long_for_an_answer_loses_whole_entries_oldest_first() {
    let request = |at: usize| json!({ "method": "GET", "status": 404, "url": format!("https://example.com/{at}/{}", "a".repeat(80)) });
    let message =
        |at: usize| json!({ "level": "error", "text": format!("m{at} {}", "b".repeat(60)) });
    let log = json!({
        "developer_mode": false,
        "requests": (0..200).map(request).collect::<Vec<_>>(),
        "console": (0..40).map(message).collect::<Vec<_>>(),
    })
    .to_string();
    assert!(
        log.chars().count() > tools::MAX_ANSWER,
        "the log fits already"
    );
    let answer = tools::page_log_answer(log, false);
    assert!(
        answer.chars().count() <= tools::MAX_ANSWER,
        "{} characters",
        answer.chars().count()
    );
    let pl3 = format!("\n\n{NEEDS_DEVELOPER_MODE}");
    assert!(answer.ends_with(&pl3), "PL3 isn't last and whole: {answer}");
    let rest = &answer[..answer.len() - pl3.len()];
    let (json, said) = rest.rsplit_once("\n\n").expect("no line before PL3");
    let left_out: usize = said
        .strip_prefix("Left out ")
        .and_then(|said| said.strip_suffix(" older entries to fit."))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("not PL5's count line: {said}"));
    let kept: Value = serde_json::from_str(json).expect("the JSON left doesn't parse");
    let (requests, console) = (
        kept["requests"].as_array().unwrap(),
        kept["console"].as_array().unwrap(),
    );
    assert_eq!(
        200 + 40 - requests.len() - console.len(),
        left_out,
        "the count is wrong"
    );
    // Whole entries, and the newest of each list kept: what went was the oldest.
    assert_eq!(requests.last(), Some(&request(199)));
    assert_eq!(console.last(), Some(&message(39)));
    let first = 200 - requests.len();
    assert!(
        (requests.iter())
            .zip(first..)
            .all(|(kept, at)| *kept == request(at)),
        "a request was cut"
    );
    // No more than it must: one entry fewer left out would pass the limit.
    let mut more = kept.clone();
    more["requests"]
        .as_array_mut()
        .unwrap()
        .insert(0, request(first - 1));
    let back = format!(
        "{more}\n\nLeft out {} older entries to fit.{pl3}",
        left_out - 1
    );
    assert!(
        back.chars().count() > tools::MAX_ANSWER,
        "it left out more than it had to"
    );
}
