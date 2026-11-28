//! A thread's look at the user's open tabs and its copy of one (#4403): titles and sites alone,
//! from the user's Spaces in the thread's profile, with private windows, other profiles, pages off
//! the web, sensitive sites and sites put off limits left out; a copy opens through open_tab in a
//! tab of the thread's own; and only the product's own threads have either, while the switch is on.

use super::kept::Run;
use super::*;
use crate::agents::runner::Cli;
use crate::agents::tools::{self, YOUR_TABS};

/// The user's tabs beside Wikipedia's (10) in Space 1: news, a site they marked sensitive, an
/// identity provider's page, the browser's own page and a site put off limits; one in a private
/// window's Space 3, and one in Space 4, in another profile.
fn users_tabs(run: &Run, cx: &mut TestAppContext) {
    run.browser.tabs.borrow_mut().extend([
        (11, 1, "Morning news", "https://news.example/today"),
        (12, 1, "Accounts", "https://mybank.example/accounts"),
        (13, 1, "Sign in", "https://www.mitid.dk/"),
        (14, 1, "Settings", "chrome://settings/"),
        (15, 1, "Payroll", "https://payroll.example/"),
        (16, 3, "Private", "https://private.example/"),
        (17, 4, "Work mail", "https://mail.example/inbox"),
    ]);
    run.browser.private.borrow_mut().push(3);
    run.browser.profiles.borrow_mut().push((4, "Profile 2"));
    run.agents.update(cx, |agents, cx| {
        agents.mark_sensitive("Default", "mybank.example", true, cx);
        agents.fence("payroll.example", cx);
    });
}

/// The thread looks: the rows of its answer.
fn look(run: &Run, thread: &str, cx: &mut TestAppContext) -> Vec<Value> {
    let answer = call(&run.agents, thread, "your_tabs", json!({}), cx);
    cx.run_until_parked();
    let answer = answer.try_recv().expect("an answer");
    assert!(!answer.error, "{}", answer.text);
    let listed: Value = serde_json::from_str(&answer.text).expect("JSON");
    listed["tabs"].as_array().expect("its tabs").clone()
}

/// The thread opens a copy of its last look's tab `n`: the answer, if one came yet.
fn copy(run: &Run, thread: &str, n: u64, cx: &mut TestAppContext) -> Option<ToolResult> {
    let answer = call(&run.agents, thread, "open_your_tab", json!({ "n": n }), cx);
    cx.run_until_parked();
    answer.try_recv().ok()
}

/// The tabs the thread's own Space has.
fn its_tabs(run: &Run, thread: &str) -> Vec<(u64, &'static str)> {
    let space = run.browser.agent_space(thread);
    (run.browser.tabs.borrow().iter())
        .filter(|tab| tab.1 == space)
        .map(|tab| (tab.0, tab.3))
        .collect()
}

/// Which of the two tools `agent`'s tools/list has, with the switch `on`.
fn listed(agent: &str, on: bool) -> Vec<String> {
    let listed = tools::listed(agent, on);
    (listed.as_array().into_iter().flatten())
        .filter_map(|tool| tool["name"].as_str())
        .filter(|name| YOUR_TABS.contains(name))
        .map(String::from)
        .collect()
}

fn stale(answer: Option<ToolResult>) {
    let answer = answer.expect("an answer");
    assert!(answer.error);
    assert_eq!(answer.text, crate::agents::your_tabs::STALE);
}

#[gpui::test]
fn a_look_lists_the_users_tabs_by_title_and_site_and_nothing_else(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-look", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    let rows = look(&run, &thread, cx);
    assert_eq!(
        rows,
        [
            json!({ "n": 1, "title": "Arc (web browser)", "site": "en.wikipedia.org" }),
            json!({ "n": 2, "title": "Morning news", "site": "news.example" }),
        ]
    );
    // Claude Code's tab in its Space, and the thread's own, aren't the user's.
    assert!(!rows.iter().any(|row| row["site"] == "example.com"));
}

/// Each look leaves one entry in the log, which counts the tabs it listed and holds no title or
/// site of the user's; its step in the trace says the look alone.
#[gpui::test]
fn each_look_is_in_the_log_by_its_count_alone(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-log", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    let looks = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| {
            (agents.activity(&thread))
                .filter(|entry| entry.action.as_ref() == "your_tabs")
                .map(|entry| {
                    let held = [&entry.site, &entry.url, &entry.said];
                    assert!(held.iter().all(|each| each.is_empty()));
                    let did = tools::Did {
                        tool: entry.action.to_string(),
                        target: entry.target.to_string(),
                        ..Default::default()
                    };
                    tools::step_title(&did, tools::Tense::Done)
                })
                .collect::<Vec<_>>()
        })
    };
    look(&run, &thread, cx);
    look(&run, &thread, cx);
    let once = "Looked at your open tabs · 2 tabs";
    assert_eq!(looks(cx), [once, once]);
    let steps = super::searches::titles(&run, &thread, cx);
    assert!(
        steps.iter().any(|step| step.ends_with("at your open tabs")),
        "{steps:?}"
    );
    // One tab, and none, as the log counts them.
    run.browser.tabs.borrow_mut().retain(|tab| tab.0 != 11);
    look(&run, &thread, cx);
    assert_eq!(looks(cx)[0], "Looked at your open tabs · 1 tab");
    run.browser.tabs.borrow_mut().retain(|tab| tab.0 != 10);
    let answer = call(&run.agents, &thread, "your_tabs", json!({}), cx);
    cx.run_until_parked();
    let answer = answer.try_recv().expect("an answer");
    assert_eq!(answer.text, crate::agents::your_tabs::NONE);
    assert_eq!(looks(cx)[0], "Looked at your open tabs · none");
}

/// A copy is open_tab's open of the page the user's tab is at, in a tab of the thread's own: the
/// user's tab stays where it is, at its page.
#[gpui::test]
fn a_copy_opens_in_a_tab_of_the_threads_and_leaves_the_users_alone(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-copy", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    look(&run, &thread, cx);
    let before = run.browser.tabs.borrow().clone();
    _ = copy(&run, &thread, 2, cx);
    assert_eq!(
        its_tabs(&run, &thread),
        [(30, "https://news.example/today")]
    );
    let users = |tabs: &[(u64, u64, &str, &str)]| -> Vec<(u64, u64, String, String)> {
        (tabs.iter())
            .filter(|tab| tab.1 != run.browser.agent_space(&thread))
            .map(|tab| (tab.0, tab.1, tab.2.into(), tab.3.into()))
            .collect()
    };
    assert_eq!(users(&run.browser.tabs.borrow()), users(&before));
    // Its step and its entry say where it came from.
    let steps = super::searches::titles(&run, &thread, cx);
    assert!(
        steps
            .iter()
            .any(|step| step.ends_with("news.example from your tabs")),
        "{steps:?}"
    );
    let opened = (run.agents).read_with(cx, |agents, _| {
        (agents.activity(&thread).next()).map(|entry| {
            let did = tools::Did {
                tool: entry.action.to_string(),
                site: entry.site.to_string(),
                target: entry.target.to_string(),
                ..Default::default()
            };
            tools::step_title(&did, tools::Tense::Done)
        })
    });
    assert_eq!(
        opened.as_deref(),
        Some("Opened news.example from your tabs")
    );
    // An open_tab of the thread's own is open_tab's, whatever its arguments say.
    let own = json!({ "url": "https://news.example/", "target": tools::FROM_YOUR_TABS });
    _ = call(&run.agents, &thread, "open_tab", own, cx);
    cx.run_until_parked();
    let steps = super::searches::titles(&run, &thread, cx);
    assert!(
        steps.last().is_some_and(|step| !step.contains("your tabs")),
        "{steps:?}"
    );
}

/// A copy reads the tab again, by every rule of the look: a number it wasn't given, a tab that
/// closed or went where it can't be listed, and a page reached by sending a form open nothing.
#[gpui::test]
fn a_copy_of_a_tab_gone_stale_opens_nothing(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-stale", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    // No look yet.
    stale(copy(&run, &thread, 1, cx));
    look(&run, &thread, cx);
    stale(copy(&run, &thread, 0, cx));
    stale(copy(&run, &thread, 3, cx));
    // News reached by a form.
    run.browser.posted.borrow_mut().insert(11);
    stale(copy(&run, &thread, 2, cx));
    // Wikipedia's tab went to a site the user marked sensitive.
    for tab in run
        .browser
        .tabs
        .borrow_mut()
        .iter_mut()
        .filter(|tab| tab.0 == 10)
    {
        tab.3 = "https://mybank.example/transfer";
    }
    stale(copy(&run, &thread, 1, cx));
    // And once closed, it's gone.
    run.browser.tabs.borrow_mut().retain(|tab| tab.0 != 10);
    stale(copy(&run, &thread, 1, cx));
    assert!(its_tabs(&run, &thread).is_empty());
}

/// Settings' switch off takes both tools from a thread's list and refuses a call to either as one
/// it doesn't have, forgets what it looked at, and stays off from one run to the next.
#[gpui::test]
fn the_switch_off_takes_both_tools_away(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-off", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    assert_eq!(listed(&thread, true), YOUR_TABS);
    assert!(listed(&thread, false).is_empty());
    look(&run, &thread, cx);
    run.agents
        .update(cx, |agents, cx| agents.set_your_tabs(false, cx));
    for tool in YOUR_TABS {
        let answer = call(&run.agents, &thread, tool, json!({ "n": 1 }), cx);
        cx.run_until_parked();
        let answer = answer.try_recv().expect("an answer");
        assert_eq!(answer.text, format!("There's no tool {tool}."));
    }
    run.agents
        .update(cx, |agents, cx| agents.set_your_tabs(true, cx));
    stale(copy(&run, &thread, 1, cx));
    assert!(its_tabs(&run, &thread).is_empty());
    run.agents
        .update(cx, |agents, cx| agents.set_your_tabs(false, cx));
    let again = Run::again(run.profile.clone(), run.program.clone(), cx);
    assert!(!again.agents.read_with(cx, |agents, _| agents.your_tabs()));
}

/// An app's agent from outside, and a thread's strand, never have either tool.
#[gpui::test]
fn only_the_products_own_threads_have_them(cx: &mut TestAppContext) {
    for agent in ["claude-code", "task-3-1"] {
        assert!(listed(agent, true).is_empty(), "{agent}");
        for tool in YOUR_TABS {
            assert!(!tools::has(agent, tool, true), "{agent} {tool}");
        }
    }
    let (agents, _, _) = connected(cx);
    let answer = call(&agents, "claude-code", "your_tabs", json!({}), cx);
    cx.run_until_parked();
    let answer = answer.try_recv().expect("an answer");
    assert_eq!(answer.text, "There's no tool your_tabs.");
}

/// A thread's look goes as it ends.
#[gpui::test]
fn a_threads_look_ends_with_it(cx: &mut TestAppContext) {
    let run = Run::new("your-tabs-end", cx);
    let thread = run.start(Cli::Claude, cx);
    users_tabs(&run, cx);
    look(&run, &thread, cx);
    let seen = |cx: &mut TestAppContext| {
        (run.agents).read_with(cx, |agents, _| {
            agents.task(&thread).unwrap().seen_tabs.len()
        })
    };
    assert_eq!(seen(cx), 2);
    run.finish(&thread, cx);
    assert_eq!(seen(cx), 0);
}
