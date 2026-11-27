//! Sensitive sites (#4369): a site the user signs in to with a bank or government ID, or marks
//! themselves, is used only at Full access, and nothing of it is sent anywhere.

use super::*;
use crate::agents::tier::{Tier, Why};

/// The user's tab 10 goes to `url`, as the core reports it.
fn goes(agents: &Entity<Agents>, browser: &Made, tab: u64, url: &str, cx: &mut TestAppContext) {
    let url: &'static str = Box::leak(url.to_string().into_boxed_str());
    for each in (browser.tabs.borrow_mut().iter_mut()).filter(|each| each.0 == tab) {
        each.3 = url;
    }
    agents.update(cx, |agents, cx| agents.sensitive_at(tab, url, cx));
}

fn marked(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<String> {
    agents.read_with(cx, |agents, _| agents.sensitive_sites("Default"))
}

fn open(agents: &Entity<Agents>, url: &str, cx: &mut TestAppContext) -> mpsc::Receiver<ToolResult> {
    call(agents, "claude-code", "open_tab", json!({ "url": url }), cx)
}

/// A tab that passes through MitID's pages and comes back marks the site it came back to, in its
/// profile, and from then on an agent's open there is refused below Full access. Nothing else
/// marks a site: the bank's own page before the sign-in, or a page after it.
#[gpui::test]
fn a_site_signed_in_to_with_a_bank_id_is_sensitive_from_then_on(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    goes(&agents, &browser, 10, "https://www.faster.example/", cx);
    assert!(marked(&agents, cx).is_empty());
    goes(
        &agents,
        &browser,
        10,
        "https://nemlog-in.mitid.dk/login",
        cx,
    );
    goes(
        &agents,
        &browser,
        10,
        "https://netbank.faster.example/overview",
        cx,
    );
    goes(&agents, &browser, 10, "https://news.example/", cx);
    assert_eq!(marked(&agents, cx), ["netbank.faster.example"]);
    refused(
        &agents,
        &browser,
        open(&agents, "netbank.faster.example", cx),
        "open netbank.faster.example",
        Why::Sensitive,
        "netbank.faster.example",
        cx,
    );
    give(&agents, Tier::Full, cx);
    let _answer = open(&agents, "netbank.faster.example", cx);
    assert!(
        browser
            .tabs
            .borrow()
            .iter()
            .any(|tab| tab.3.contains("faster") && tab.0 != 10),
        "it didn't open at Full access"
    );
}

/// A sign-in in a window of its own, a tab the site's page opened, marks the site that opened it.
#[gpui::test]
fn a_sign_in_window_marks_the_site_that_opened_it(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    goes(&agents, &browser, 10, "https://skat.example/", cx);
    (browser.tabs.borrow_mut()).push((31, 1, "MitID", "about:blank"));
    agents.update(cx, |agents, cx| agents.tab_opened(31, 10, cx));
    goes(&agents, &browser, 31, "https://www.mitid.dk/", cx);
    assert_eq!(marked(&agents, cx), ["skat.example"]);
}

/// The identity provider's own pages are a sign-in, which an agent below Full access can't do,
/// so it can't sign in to mark a site either.
#[gpui::test]
fn an_agent_cant_sign_in_with_a_bank_id_below_full_access(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    refused(
        &agents,
        &browser,
        open(&agents, "www.mitid.dk", cx),
        "open mitid.dk",
        Why::SignInSensitive,
        "mitid.dk",
        cx,
    );
    assert!(marked(&agents, cx).is_empty());
}

/// The user's own mark makes a site sensitive, its Remove undoes it, and a later sign-in with a
/// bank or government ID marks it again (SS8). None of it reaches the core or any site.
#[gpui::test]
fn the_users_mark_and_its_remove(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| {
        agents.mark_sensitive("Default", "clinic.example", true, cx)
    });
    assert_eq!(marked(&agents, cx), ["clinic.example"]);
    refused(
        &agents,
        &browser,
        open(&agents, "www.clinic.example", cx),
        "open clinic.example",
        Why::Sensitive,
        "clinic.example",
        cx,
    );
    agents.update(cx, |agents, cx| {
        agents.mark_sensitive("Default", "clinic.example", false, cx)
    });
    assert!(marked(&agents, cx).is_empty());
    goes(&agents, &browser, 10, "https://account.gov.uk/sign-in", cx);
    goes(&agents, &browser, 10, "https://clinic.example/records", cx);
    assert_eq!(marked(&agents, cx), ["clinic.example"]);
    // Another profile keeps its own.
    assert!(
        agents
            .read_with(cx, |agents, _| agents.sensitive_sites("Profile 2"))
            .is_empty()
    );
    assert!(browser.reads.borrow().is_empty());
    assert!(browser.requests.borrow().is_empty());
    assert!(browser.saved_reads.borrow().is_empty());
}
