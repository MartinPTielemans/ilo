//! The user's profiles an agent works in (#4376): the one it names, else the core's pick by
//! history, else the user's window's; only those the user leaves checked; and why, on the card and
//! in the log, never in an answer.

use super::*;
use crate::agents::profiles::{self, OpenedIn, THREADS};
use crate::agents::ready::{Found, Ready};
use crate::agents::task::State;
use crate::agents::tier::{Tier, Why};

/// Claude Code, connected, with the user's profiles Work (the core's first), Home and Bank, whose
/// Spaces of the user's are 1, 5 and 6; it has no Space of its own yet.
fn three(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
    let browser = Rc::new(Made::default());
    browser.profile_list.borrow_mut().extend([
        (1, "Default", "Work"),
        (2, "Profile 2", "Home"),
        (3, "Profile 3", "Bank"),
    ]);
    (browser.profiles.borrow_mut()).extend([(5, "Profile 2"), (6, "Profile 3")]);
    (browser.tabs.borrow_mut()).push((10, 1, "Arc", "https://en.wikipedia.org/wiki/Arc"));
    let configs = Arc::new(Configs::default());
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    (agents, browser)
}

/// Claude Code's open_tab of `url`, in `profile` if it names one.
fn open(
    agents: &Entity<Agents>,
    url: &str,
    profile: Option<&str>,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    let mut arguments = json!({ "url": url });
    if let Some(profile) = profile {
        arguments["profile"] = json!(profile);
    }
    call(agents, "claude-code", "open_tab", arguments, cx)
}

/// The pick the core was asked last: the profiles offered and its request.
fn last_pick(browser: &Made) -> (Vec<u64>, u64) {
    let picks = browser.picks.borrow();
    let (offered, _, request) = picks.last().expect("no pick asked").clone();
    (offered, request)
}

/// The core picks `profile` (0 for none) for `request`.
fn picked(agents: &Entity<Agents>, request: u64, profile: u64, cx: &mut TestAppContext) {
    agents.update(cx, |agents, cx| agents.profile_picked(request, profile, cx));
}

/// The Space `tab` is in, and that Space's profile's folder.
fn where_is(browser: &Made, tab: u64) -> (u64, String) {
    let space = browser
        .tabs
        .borrow()
        .iter()
        .find(|each| each.0 == tab)
        .unwrap()
        .1;
    (space, browser.space_profile(space))
}

/// Where the log's newest entry of Claude Code's says its tab opened.
fn logged_in(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Option<OpenedIn> {
    agents.read_with(cx, |agents, _| {
        agents
            .activity("claude-code")
            .next()
            .unwrap()
            .opened_in
            .clone()
    })
}

/// The line under the open_tab step of Claude Code's session, as its card says it.
fn card_line(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Option<SharedString> {
    agents.read_with(cx, |agents, _| {
        let session = (agents.tasks()).find(|task| {
            (task.session.as_ref()).is_some_and(|session| session.agent == "claude-code")
        });
        let step = (session?.trace.iter().rev()).find(|step| step.did.tool == "open_tab");
        step?.opened_in.clone()
    })
}

fn choices(agents: &Entity<Agents>, key: &str, cx: &mut TestAppContext) -> Vec<(String, bool)> {
    agents.read_with(cx, |agents, _| {
        (agents.profile_choices(key).into_iter())
            .map(|(profile, on)| (profile.name, on))
            .collect()
    })
}

#[gpui::test]
fn a_named_profile_opens_there(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    let answer = open(&agents, "example.com", Some("Home"), cx);
    assert!(
        browser.picks.borrow().is_empty(),
        "the core was asked to pick"
    );
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert!(
        answer
            .text
            .starts_with("Opened tab 40 (profile: \"Home\")."),
        "{}",
        answer.text
    );
    assert_eq!(where_is(&browser, 40), (200, "Profile 2".into()));
    // Its Space there is named for the app and the profile (PR6), beside the one its session
    // started with in the first profile it may use.
    assert_eq!(
        *browser.made.borrow(),
        [
            ("claude-code".to_string(), "Claude Code · Work".to_string()),
            ("claude-code".to_string(), "Claude Code · Home".to_string())
        ]
    );
    let opened = logged_in(&agents, cx).expect("the log says nothing of the profile");
    assert_eq!(opened.why, "as Claude Code asked");
    // list_tabs says each tab's profile.
    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    let listed = listed.recv().unwrap();
    assert!(
        listed.text.contains("  profile: \"Home\""),
        "{}",
        listed.text
    );
}

#[gpui::test]
fn an_unknown_and_a_switched_off_name_get_the_same_answer(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed("claude-code", "Profile 3", false, cx)
    });
    let answers: Vec<ToolResult> = ["Bank", "Nowhere"]
        .into_iter()
        .map(|name| {
            let answer = open(&agents, "example.com", Some(name), cx);
            answer.try_recv().expect("not answered at once")
        })
        .collect();
    for (answer, name) in answers.iter().zip(["Bank", "Nowhere"]) {
        assert!(answer.error);
        assert!(
            answer.text.contains(&profiles::cant_use(false, name)),
            "{}",
            answer.text
        );
    }
    assert_eq!(
        answers[0].text.replace("Bank", "_"),
        answers[1].text.replace("Nowhere", "_"),
        "a profile that exists is told from one that doesn't"
    );
    assert!(
        (browser.made.borrow().iter()).all(|(_, name)| name == "Claude Code · Work"),
        "a Space was made in another profile"
    );
    assert!(browser.pages.borrow().is_empty(), "a tab opened");
}

#[gpui::test]
fn a_picked_open_uses_the_hosts_pick_then_the_window_rule(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    let answer = open(&agents, "https://example.com/", None, cx);
    assert!(
        browser.pages.borrow().is_empty(),
        "it opened before the pick"
    );
    let (offered, request) = last_pick(&browser);
    assert_eq!(offered, [1, 2, 3]);
    picked(&agents, request, 2, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert_eq!(where_is(&browser, 40).1, "Profile 2");
    assert_eq!(
        logged_in(&agents, cx).unwrap().why,
        "where you last used it"
    );

    // No history of it anywhere: the profile of the user's window, Bank's here.
    browser.front.set(Some(6));
    let answer = open(&agents, "https://example.org/", None, cx);
    let (_, request) = last_pick(&browser);
    picked(&agents, request, 0, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert!(
        answer.text.contains("(profile: \"Bank\")"),
        "{}",
        answer.text
    );
    assert_eq!(where_is(&browser, 41).1, "Profile 3");
    assert_eq!(
        logged_in(&agents, cx).unwrap().why,
        "your current window's profile"
    );

    // The window's profile is off for the app: the first it may use, and the log says that, not
    // the window's (PR3b).
    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed("claude-code", "Profile 3", false, cx)
    });
    let answer = open(&agents, "https://example.net/", None, cx);
    let (offered, request) = last_pick(&browser);
    assert_eq!(offered, [1, 2]);
    picked(&agents, request, 0, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert!(
        answer.text.contains("(profile: \"Work\")"),
        "{}",
        answer.text
    );
    assert_eq!(
        logged_in(&agents, cx).unwrap().why,
        "the first profile it may use"
    );
}

#[gpui::test]
fn a_switched_off_profile_is_neither_listed_nor_picked(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed("claude-code", "Profile 2", false, cx)
    });
    let listed = call(&agents, "claude-code", "list_profiles", json!({}), cx);
    assert_eq!(listed.recv().unwrap().text, "Work\nBank");
    // Neither offered to the pick nor taken from the user's window in it.
    browser.front.set(Some(5));
    let answer = open(&agents, "https://example.com/", None, cx);
    let (offered, request) = last_pick(&browser);
    assert_eq!(offered, [1, 3]);
    picked(&agents, request, 2, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(
        answer.text.contains("(profile: \"Work\")"),
        "{}",
        answer.text
    );
    assert_eq!(where_is(&browser, 30), (2, "Default".into()));
}

#[gpui::test]
fn the_answer_holds_the_profile_name_only(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    let answer = open(&agents, "https://example.com/", None, cx);
    let (_, request) = last_pick(&browser);
    picked(&agents, request, 2, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(answer.text.contains("profile: \"Home\""), "{}", answer.text);
    for word in [
        "last used",
        "history",
        "window",
        "asked",
        "visited",
        "where you",
    ] {
        assert!(!answer.text.contains(word), "{word} in: {}", answer.text);
    }
    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    let listed = listed.recv().unwrap().text;
    assert!(!listed.contains("last used"), "{listed}");
}

#[gpui::test]
fn the_card_and_the_log_hold_the_why(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    let answer = open(&agents, "https://example.com/", None, cx);
    let (_, request) = last_pick(&browser);
    picked(&agents, request, 2, cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    assert_eq!(
        card_line(&agents, cx).as_deref(),
        Some("In Home, where you last used it")
    );
    let opened = logged_in(&agents, cx).unwrap();
    assert_eq!(
        opened.log_line(Some("Claude Code"), "example.com"),
        "Claude Code opened example.com in Home · where you last used it"
    );
    assert_eq!(
        opened.log_line(None, "example.com"),
        "Opened example.com in Home · where you last used it"
    );
}

/// The core lists no private window's profile, and a private window in front is never the
/// window rule's.
#[gpui::test]
fn no_private_profile_is_listed_or_offered(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    // A private window of Home's, in front.
    (browser.profiles.borrow_mut()).push((7, "Profile 2"));
    browser.private.borrow_mut().push(7);
    browser.front.set(Some(7));
    let listed = call(&agents, "claude-code", "list_profiles", json!({}), cx);
    assert_eq!(listed.recv().unwrap().text, "Work\nHome\nBank");
    let answer = open(&agents, "https://example.com/", None, cx);
    let (offered, request) = last_pick(&browser);
    assert_eq!(offered, [1, 2, 3]);
    picked(&agents, request, 0, cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(
        answer.text.contains("(profile: \"Work\")"),
        "{}",
        answer.text
    );
}

#[gpui::test]
fn switching_a_profile_off_hands_its_tabs_back(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    let answer = open(&agents, "example.com", Some("Home"), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    assert_eq!(where_is(&browser, 40).0, 200);
    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed("claude-code", "Profile 2", false, cx)
    });
    assert_eq!(where_is(&browser, 40).0, 5, "the tab stayed the app's");
    assert_eq!(*browser.deleted.borrow(), [200]);
    assert_eq!(
        choices(&agents, "claude-code", cx),
        [
            ("Work".into(), true),
            ("Home".into(), false),
            ("Bank".into(), true)
        ]
    );
    // The last one on stays on.
    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed("claude-code", "Profile 3", false, cx);
        agents.set_profile_allowed("claude-code", "Default", false, cx);
    });
    assert_eq!(
        choices(&agents, "claude-code", cx),
        [
            ("Work".into(), true),
            ("Home".into(), false),
            ("Bank".into(), false)
        ]
    );
    // Threads keep a list of their own.
    assert!(choices(&agents, THREADS, cx).iter().all(|(_, on)| *on));
}

/// Only the user's click sets the lists: no tool is named for one, and an agent's calls leave
/// them as they were.
#[gpui::test]
fn no_tool_writes_the_list(cx: &mut TestAppContext) {
    for agent in ["claude-code", "task-1"] {
        let named: Vec<String> = (tools::names(agent).into_iter())
            .filter(|name| name.contains("profile"))
            .collect();
        assert_eq!(named, ["list_profiles"], "{agent}");
    }
    let (agents, browser) = three(cx);
    let before = choices(&agents, "claude-code", cx);
    call(&agents, "claude-code", "list_profiles", json!({}), cx);
    let answer = open(&agents, "example.com", Some("Bank"), cx);
    page_done(&agents, &browser, answer, shop_tree(), cx);
    assert_eq!(choices(&agents, "claude-code", cx), before);
}

/// With one profile, nothing names one: not the answer, the log, the card or the Space, and
/// Settings draws no checkmarks; list_profiles still names it.
#[gpui::test]
fn with_one_profile_nothing_names_one(cx: &mut TestAppContext) {
    let (browser, configs) = (Rc::new(Made::default()), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, None, cx);
    agents.update(cx, |agents, cx| agents.connect(Client::ClaudeCode, cx));
    cx.run_until_parked();
    let answer = open(&agents, "example.com", None, cx);
    assert!(
        browser.picks.borrow().is_empty(),
        "the core was asked to pick"
    );
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(answer.text.starts_with("Opened tab 30."), "{}", answer.text);
    assert!(!answer.text.contains("profile"), "{}", answer.text);
    assert_eq!(
        *browser.made.borrow(),
        [("claude-code".to_string(), "Claude Code".to_string())]
    );
    assert_eq!(logged_in(&agents, cx), None);
    assert_eq!(card_line(&agents, cx), None);
    assert!(!agents.read_with(cx, |agents, _| agents.several_profiles()));
    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx);
    assert!(!listed.recv().unwrap().text.contains("profile"));
    let listed = call(&agents, "claude-code", "list_profiles", json!({}), cx);
    assert_eq!(listed.recv().unwrap().text, "Default");
    // Named, it opens there just the same.
    let answer = open(&agents, "example.org", Some("Default"), cx);
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(!answer.error, "{}", answer.text);
    assert!(!answer.text.contains("profile"), "{}", answer.text);
}

/// Off limits is the user's in every profile: an open there is refused whichever it names.
#[gpui::test]
fn the_same_rules_in_every_profile(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    agents.update(cx, |agents, cx| {
        assert!(agents.fence("https://mybank.example/", cx).is_some());
    });
    for profile in ["Work", "Home", "Bank"] {
        let answer = open(&agents, "https://mybank.example/", Some(profile), cx);
        let site = "mybank.example";
        refused(&agents, &browser, answer, site, Why::Fenced, site, cx);
    }
    assert!(browser.pages.borrow().is_empty(), "a tab opened");
    assert!(
        browser.tabs.borrow().iter().all(|tab| tab.1 == 1),
        "a tab opened"
    );
}

/// An open of an Off limits site that names no profile is refused without asking any profile's
/// history where the user last went there.
#[gpui::test]
fn an_off_limits_site_reads_no_history(cx: &mut TestAppContext) {
    let (agents, browser) = three(cx);
    agents.update(cx, |agents, cx| {
        assert!(agents.fence("https://mybank.example/", cx).is_some());
    });
    let answer = open(&agents, "https://mybank.example/", None, cx);
    let site = "mybank.example";
    refused(&agents, &browser, answer, site, Why::Fenced, site, cx);
    assert!(
        browser.picks.borrow().is_empty(),
        "the core was asked for a pick for an Off limits site: {:?}",
        browser.picks.borrow()
    );
}

/// A thread of Claude's at work, asked in the user's Space 5, Home's.
fn thread(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, String) {
    let (agents, browser) = three(cx);
    agents.update(cx, |agents, cx| {
        agents.set_threads(true, cx);
        let found = Found {
            ready: Ready::SignedIn,
            ..Default::default()
        };
        agents.tasks.found.insert(Cli::Claude, found);
        agents.tasks.default = Some(Cli::Claude);
    });
    let agent = agents.update(cx, |agents, cx| {
        agents.start_task(Cli::Claude, "Plan a walk", None, 5, Tier::default(), cx)
    });
    let agent = agent.expect("started");
    agents.update(cx, |agents, _| {
        let task = agents.task_mut(&agent).unwrap();
        task.ended = None;
        task.state = State::Working;
    });
    (agents, browser, agent)
}

/// A thread opens in its own Space's profile unless it names another its list has on, and is
/// told so in its own words.
#[gpui::test]
fn a_thread_opens_in_its_own_spaces_profile(cx: &mut TestAppContext) {
    let (agents, browser, agent) = thread(cx);
    let url = json!({ "url": "example.com" });
    let answer = call(&agents, &agent, "open_tab", url, cx);
    assert!(
        browser.picks.borrow().is_empty(),
        "the core was asked to pick"
    );
    let answer = page_done(&agents, &browser, answer, shop_tree(), cx);
    assert!(
        answer.text.contains("(profile: \"Home\")"),
        "{}",
        answer.text
    );
    let (_, folder) = where_is(&browser, 40);
    assert_eq!(folder, "Profile 2");

    agents.update(cx, |agents, cx| {
        agents.set_profile_allowed(THREADS, "Profile 3", false, cx)
    });
    let bank = json!({ "url": "example.com", "profile": "Bank" });
    let refused = call(&agents, &agent, "open_tab", bank, cx).recv().unwrap();
    assert!(refused.error);
    assert!(
        refused.text.contains(&profiles::cant_use(true, "Bank")),
        "{}",
        refused.text
    );
    // The app's list is its own.
    assert!(
        choices(&agents, "claude-code", cx)
            .iter()
            .all(|(_, on)| *on)
    );
}
