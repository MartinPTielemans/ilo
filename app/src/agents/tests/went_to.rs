//! The activity log names a navigate, a back and a forward by the site the tab went to, as the
//! core has its address once the page answers, and not by the one it left, which is all that's
//! known when the call is let go.

use super::*;

/// The core has `tab` at `url` from now on, as after a page there committed.
fn at(browser: &Made, tab: u64, url: &'static str) {
    let mut tabs = browser.tabs.borrow_mut();
    tabs.iter_mut().find(|each| each.0 == tab).unwrap().3 = url;
}

/// The log's newest entry of Claude Code's: its action, its site and why it was refused.
fn newest(agents: &Entity<Agents>, cx: &mut TestAppContext) -> (String, String, String) {
    log(agents, "claude-code", cx).remove(0)
}

fn entry(action: &str, site: &str) -> (String, String, String) {
    (action.into(), site.into(), String::new())
}

/// What Claude Code is doing, as the bar in its Space says it.
fn doing(agents: &Entity<Agents>, cx: &mut TestAppContext) -> String {
    agents.read_with(cx, |agents, _| agents.doing["claude-code"].to_string())
}

/// Tab 20, at example.com, is sent `url` by navigate, and its page answers once the core has it
/// at `lands`. Under way its entry, `action`, names no site, and then the one it landed on.
fn goes_to(url: &str, action: &str, lands: &'static str, site: &str, cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let going = json!({ "tab": 20, "url": url });
    let answer = call(&agents, "claude-code", "navigate", going, cx);
    assert_eq!(newest(&agents, cx), entry(action, ""));
    let did = tools::describe(action);
    assert_eq!(doing(&agents, cx), did);

    at(&browser, 20, lands);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert_eq!(newest(&agents, cx), entry(action, site));
    assert_eq!(doing(&agents, cx), format!("{did} · {site}"));
    assert_eq!(log(&agents, "claude-code", cx).len(), 2, "one entry, named");
}

/// A navigate is named by where the core has the tab once its page answers: not example.com,
/// which it left, and not iana.org, which the agent wrote and which sent it on.
#[gpui::test]
fn a_navigate_is_named_by_the_site_its_tab_went_to(cx: &mut TestAppContext) {
    let lands = "https://www.icann.org/resources?from=iana";
    goes_to("iana.org", "navigate", lands, "icann.org", cx);
}

#[gpui::test]
fn a_back_is_named_by_the_site_its_tab_went_to(cx: &mut TestAppContext) {
    goes_to(
        "back",
        "back",
        "https://www.iana.org/domains",
        "iana.org",
        cx,
    );
}

#[gpui::test]
fn a_forward_is_named_by_the_site_its_tab_went_to(cx: &mut TestAppContext) {
    goes_to(
        "forward",
        "forward",
        "https://www.iana.org/numbers",
        "iana.org",
        cx,
    );
}

/// A refused one never goes, so it keeps naming what was refused: the site of the tab the agent
/// may not move, or of the address it may not open.
#[gpui::test]
fn a_refused_navigate_names_what_was_refused(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    browser.held.borrow_mut().insert(20);
    for url in ["iana.org", "back", "forward"] {
        let going = json!({ "tab": 20, "url": url });
        let answer = call(&agents, "claude-code", "navigate", going, cx);
        assert!(answer.recv().unwrap().error);
        let action = if url == "iana.org" { "navigate" } else { url };
        let refused = (action.into(), "example.com".into(), HELD.into());
        assert_eq!(newest(&agents, cx), refused);
    }
    browser.held.borrow_mut().remove(&20);

    let file = json!({ "tab": 20, "url": "file:///etc/hosts" });
    let answer = call(&agents, "claude-code", "navigate", file, cx);
    assert!(answer.recv().unwrap().error);
    let (action, site, refused) = newest(&agents, cx);
    assert_eq!((action.as_str(), site.as_str()), ("navigate", "file"));
    assert!(!refused.is_empty());
    assert!(
        browser.pages.borrow().is_empty(),
        "nothing was asked of a page"
    );
}

/// A run's step that goes somewhere names its own entry, among the run's steps.
#[gpui::test]
fn a_runs_step_is_named_by_the_site_its_tab_went_to(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let steps = json!({ "tab": 20, "steps": [{ "action": "navigate", "url": "back" }] });
    let run = call(&agents, "claude-code", "run_steps", steps, cx);
    let steps = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            let run = agents.activity("claude-code").next().unwrap();
            assert_eq!(run.action.as_ref(), "run_steps");
            let steps = run.steps.iter();
            steps
                .map(|step| (step.action.to_string(), step.site.to_string()))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(steps(cx), [("back".to_string(), String::new())]);
    at(&browser, 20, "https://www.iana.org/domains");
    let ran = page_done(&agents, &browser, run, shop_tree(), cx);
    assert!(!ran.error, "{}", ran.text);
    assert_eq!(steps(cx), [("back".to_string(), "iana.org".to_string())]);
}
