//! An agent's sign_in with a login the user saved (#4098), on a browser that makes up the core's
//! answers: what the app checks before it asks the core, what the agent hears, and that neither
//! the account nor a withheld page's title or address reaches what an agent reads or the app keeps.
//! Every value is made up.

use super::*;
use crate::agents::task::Line;
use crate::agents::task::ilo::{Account, IloKind, IloLine, SignedInWith};
use crate::agents::tier::Tier;

const ACCOUNT: &str = "made-up-account-4121@example.org";
/// In the title and address of the page a sign-in's form went to, which the core withholds.
const MARKER: &str = "made-up-marker-4121";
/// What the core says of a sign-in that went, in its words, which name no account.
const SIGNED: &str = "ilo filled the login saved for this site and sent the form. Read the page.";
const USER_STEP: &str = "The site asks for a step only the user can take.";

/// Connected, with Settings' switch set on by the user, as each test here but the switch's own
/// starts: it's off until they set it.
fn connected(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, Arc<Configs>) {
    let (agents, browser, configs) = super::connected(cx);
    agents.update(cx, |agents, cx| agents.set_sign_in_on(true, cx));
    (agents, browser, configs)
}

fn hello(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    let event = Event::Hello {
        agent: "claude-code".into(),
        connection: 1,
        client: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(event, cx));
}

fn bye(agents: &Entity<Agents>, cx: &mut TestAppContext) {
    let closed = Event::Closed {
        agent: "claude-code".into(),
        connection: 1,
    };
    agents.update(cx, |agents, cx| agents.handle(closed, cx));
    let bye = Event::Bye {
        agent: "claude-code".into(),
    };
    agents.update(cx, |agents, cx| agents.handle(bye, cx));
}

fn sign_in(
    agents: &Entity<Agents>,
    tab: u64,
    cx: &mut TestAppContext,
) -> mpsc::Receiver<ToolResult> {
    call(agents, "claude-code", "sign_in", json!({ "tab": tab }), cx)
}

/// The request of the last sign-in the core was asked for.
fn asked(browser: &Made) -> u64 {
    browser
        .saved_sign_ins
        .borrow()
        .last()
        .expect("a sign-in asked of the core")
        .2
}

fn done(
    agents: &Entity<Agents>,
    request: u64,
    signed_in: bool,
    text: &str,
    failure: Option<Failure>,
    cx: &mut TestAppContext,
) {
    let text = text.to_string();
    agents.update(cx, |agents, cx| {
        agents.sign_in_done(request, signed_in, true, text, failure, cx)
    });
}

/// Tab `tab`'s page is now `url`, titled `title`.
fn at(browser: &Made, tab: u64, title: &str, url: &str) {
    let (title, url): (&'static str, &'static str) = (
        Box::leak(title.to_string().into_boxed_str()),
        Box::leak(url.to_string().into_boxed_str()),
    );
    for each in (browser.tabs.borrow_mut().iter_mut()).filter(|each| each.0 == tab) {
        (each.2, each.3) = (title, url);
    }
}

/// Past Settings' switch, the Off limits list and the task's access, the core signs the tab in;
/// the agent hears the core's words, and the log has the ask and the fill, neither with the account.
#[gpui::test]
fn a_sign_in_past_the_apps_checks_is_the_cores_in_its_words(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let answer = sign_in(&agents, 20, cx);
    assert!(browser.checked.borrow().contains(&"sign_in".to_string()));
    let request = asked(&browser);
    assert_eq!(browser.saved_sign_ins.borrow()[0].1, 20);
    assert!(answer.try_recv().is_err(), "the call waits on the core");

    agents.update(cx, |agents, cx| {
        agents.signed_in("claude-code", 20, "example.com", ACCOUNT.into(), cx)
    });
    done(&agents, request, true, SIGNED, None, cx);
    let answer = answer.recv().unwrap();
    assert!(!answer.error);
    assert_eq!(answer.text, SIGNED);
    // Newest first, above the connection's own line.
    let logged = log(&agents, "claude-code", cx)[..2].to_vec();
    let actions: Vec<&str> = logged.iter().map(|(action, ..)| action.as_str()).collect();
    assert_eq!(actions, ["filled_login", "sign_in"]);
    assert!(
        logged
            .iter()
            .all(|(_, site, refused)| site == "example.com" && refused.is_empty())
    );

    // Not signed in: an error, in the core's words.
    let again = sign_in(&agents, 20, cx);
    let words = "The site didn't take the login saved for it.";
    done(&agents, asked(&browser), false, words, None, cx);
    let again = again.recv().unwrap();
    assert!(again.error);
    assert_eq!(again.text, words);
}

/// Off in Settings, no sign-in reaches the core, and the agent hears it's the user's to change.
#[gpui::test]
fn sign_in_for_agents_off_asks_nothing_of_the_core(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    agents.update(cx, |agents, cx| agents.set_sign_in_on(false, cx));
    let answer = sign_in(&agents, 20, cx).recv().unwrap();
    assert!(answer.error);
    assert_eq!(answer.kind, Some(tools::Kind::Outside));
    assert!(browser.saved_sign_ins.borrow().is_empty());
    assert_eq!(log(&agents, "claude-code", cx)[0].0, "sign_in");
    assert!(
        !log(&agents, "claude-code", cx)[0].2.is_empty(),
        "logged as refused"
    );
}

/// The task's access decides as for any act, before the core: a site on the Off limits list at
/// every tier, a bank's site above Work on pages, and any site at Look only. Full access signs in
/// on a bank's site.
#[gpui::test]
fn the_tasks_access_and_the_off_limits_list_decide_before_the_core(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let asked_of_core = || browser.saved_sign_ins.borrow().len();
    at(&browser, 20, "Forum", "https://forum.example.net/login");
    agents.update(cx, |agents, cx| {
        assert!(agents.fence("forum.example.net", cx).is_some());
    });
    let fenced = sign_in(&agents, 20, cx).recv().unwrap();
    assert!(fenced.error);
    assert_eq!(fenced.kind, Some(tools::Kind::Outside));

    at(&browser, 20, "My Bank", "https://www.mybank.example/login");
    let bank = sign_in(&agents, 20, cx).recv().unwrap();
    assert!(bank.error);
    assert_eq!(bank.kind, Some(tools::Kind::Outside));
    assert_ne!(bank.text, fenced.text);
    assert_eq!(asked_of_core(), 0);

    let key = agents.read_with(cx, |agents, cx| agents.shown_cards(cx)[0].agent.to_string());
    agents.update(cx, |agents, cx| agents.set_tier(&key, Tier::Full, cx));
    let full = sign_in(&agents, 20, cx);
    assert_eq!(asked_of_core(), 1, "Full access signs in on a bank's site");
    done(&agents, asked(&browser), true, SIGNED, None, cx);
    assert!(!full.recv().unwrap().error);

    agents.update(cx, |agents, cx| agents.set_tier(&key, Tier::Look, cx));
    at(&browser, 20, "Example Domain", "https://www.example.com/");
    let look = sign_in(&agents, 20, cx).recv().unwrap();
    assert!(look.error);
    assert_eq!(look.kind, Some(tools::Kind::Outside));
    assert_eq!(asked_of_core(), 1);
}

/// A step only the user can take hands them the tab, as a handoff does; the call waits, and its
/// answer once they hand it back starts with the core's words. A tab the core holds already, as
/// its page showed what was filled, is handed over as it is.
#[gpui::test]
fn the_users_step_hands_them_the_tab_and_answers_once_its_back(cx: &mut TestAppContext) {
    for core_holds in [false, true] {
        let (agents, browser, _) = connected(cx);
        let answer = sign_in(&agents, 20, cx);
        if core_holds {
            browser.held.borrow_mut().insert(20);
        }
        let failure = Some(Failure::UserStep);
        done(&agents, asked(&browser), false, USER_STEP, failure, cx);
        assert!(browser.held.borrow().contains(&20));
        assert!(answer.try_recv().is_err(), "waits for the user");
        let actions: Vec<String> = (log(&agents, "claude-code", cx).into_iter())
            .map(|(action, ..)| action)
            .collect();
        assert_eq!(actions[0], "hand_off");

        agents.update(cx, |agents, cx| agents.hand_back(20, cx));
        let answer = answer.recv().unwrap();
        assert!(answer.text.starts_with(USER_STEP), "{}", answer.text);
        assert!(!answer.error);
        assert!(browser.held.borrow().is_empty());
    }
}

/// The form went to a page the core withholds, whose title and address hold the marker. The host
/// tells the app of the commit before the agent asks anything, and the answers are still
/// withheld: each act is refused in the core's words, which say only to move the page on.
/// Nothing the app keeps or shows for agents holds the marker: the 30-day record, the activity
/// log, the card, the conversation or an answer. The account is the conversation's ilo line's
/// alone.
#[gpui::test]
fn a_withheld_page_and_the_account_reach_nothing_an_agent_reads_or_the_app_keeps(
    cx: &mut TestAppContext,
) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    // An outline of the sign-in page first, whose refs a later click or type could name.
    let outline = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    page_done(&agents, &browser, outline, shop_tree(), cx);
    let pages = browser.pages.borrow().len();
    let answer = sign_in(&agents, 20, cx);
    let request = asked(&browser);
    agents.update(cx, |agents, cx| {
        agents.signed_in("claude-code", 20, "example.com", ACCOUNT.into(), cx)
    });
    let url = format!("https://www.example.com/welcome?{MARKER}");
    at(&browser, 20, &format!("Welcome {MARKER}"), &url);
    browser.withheld.borrow_mut().insert(20);
    agents.update(cx, |agents, _| agents.tab_at(20, &url));
    done(&agents, request, true, SIGNED, None, cx);
    let mut answers = vec![answer.recv().unwrap()];

    let listed = call(&agents, "claude-code", "list_tabs", json!({}), cx)
        .recv()
        .unwrap();
    assert!(listed.text.contains(WITHHELD), "{}", listed.text);
    answers.push(listed);
    let page_acts = [
        ("read_page", json!({ "tab": 20 })),
        ("snapshot", json!({ "tab": 20 })),
        ("screenshot", json!({ "tab": 20 })),
        ("click", json!({ "tab": 20, "ref": "e1" })),
        ("type", json!({ "tab": 20, "ref": "e1", "text": "hello" })),
        ("sign_in", json!({ "tab": 20 })),
    ];
    for (tool, arguments) in page_acts {
        let refused = call(&agents, "claude-code", tool, arguments, cx)
            .recv()
            .unwrap();
        assert!(refused.error, "{tool}");
        assert_eq!(refused.text, WITHHELD_REFUSAL, "{tool}");
        answers.push(refused);
    }
    assert_eq!(
        browser.pages.borrow().len(),
        pages,
        "nothing reached the page"
    );
    assert!(browser.reads.borrow().is_empty(), "nothing read its text");
    assert_eq!(browser.saved_sign_ins.borrow().len(), 1);
    for answer in &answers {
        assert!(!format!("{answer:?}").contains(MARKER), "{answer:?}");
        assert!(!format!("{answer:?}").contains(ACCOUNT), "{answer:?}");
    }

    let shown = agents.read_with(cx, |agents, cx| {
        let cards = agents.shown_cards(cx);
        let talk = (cards.first()).and_then(|card| agents.conversation(&card.agent, cx));
        (cards, talk)
    });
    let (cards, talk) = shown;
    assert!(!cards.is_empty(), "its card");
    let talk = talk.expect("its conversation");
    assert!(!format!("{cards:?}").contains(MARKER), "{cards:?}");
    assert!(!format!("{cards:?}").contains(ACCOUNT), "{cards:?}");
    assert!(!format!("{talk:?}").contains(MARKER), "{talk:?}");
    let filled = (talk.lines.iter()).find_map(|line| match line {
        Line::Ilo(line) => match &line.kind {
            IloKind::SignedIn {
                with: SignedInWith::Saved(account),
                site,
                ..
            } => Some((account.drawn().to_string(), site.to_string())),
            _ => None,
        },
        _ => None,
    });
    assert_eq!(filled, Some((ACCOUNT.into(), "example.com".into())));

    // Taking the tab to another page is what the agent may still do.
    let to = json!({ "tab": 20, "url": "https://example.org/" });
    drop(call(&agents, "claude-code", "navigate", to, cx));
    assert_eq!(
        browser.navigated.borrow().last().map(|(tab, _)| *tab),
        Some(20)
    );

    let activity: Vec<Activity> = agents.read_with(cx, |agents, _| {
        agents.activity("claude-code").cloned().collect()
    });
    assert!(!format!("{activity:?}").contains(MARKER), "{activity:?}");
    assert!(!format!("{activity:?}").contains(ACCOUNT), "{activity:?}");

    bye(&agents, cx);
    let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
    assert!(!kept.is_empty(), "its 30-day record");
    assert!(!format!("{kept:?}").contains(MARKER), "{kept:?}");
    assert!(!format!("{kept:?}").contains(ACCOUNT), "{kept:?}");
}

/// A fill whose form wasn't sent: the core answers without telling the app it signed in, and
/// neither the conversation nor the activity log says ilo filled and sent the form.
#[gpui::test]
fn a_fill_that_was_not_sent_adds_no_ilo_line_and_no_entry(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let answer = sign_in(&agents, 20, cx);
    let words = "ilo filled the login saved for this site, but the form didn't go.";
    agents.update(cx, |agents, cx| {
        agents.sign_in_done(asked(&browser), false, false, words.into(), None, cx)
    });
    let answer = answer.recv().unwrap();
    assert!(answer.error);
    assert_eq!(answer.text, words);

    let actions: Vec<String> = log(&agents, "claude-code", cx)
        .into_iter()
        .map(|(action, ..)| action)
        .collect();
    assert!(actions.contains(&"sign_in".to_string()), "{actions:?}");
    assert!(
        !actions.contains(&"filled_login".to_string()),
        "{actions:?}"
    );
    let talk = agents.read_with(cx, |agents, cx| {
        let cards = agents.shown_cards(cx);
        (cards.first()).and_then(|card| agents.conversation(&card.agent, cx))
    });
    let talk = talk.expect("its conversation");
    let filled = (talk.lines.iter()).any(|line| {
        matches!(
            line,
            Line::Ilo(IloLine {
                kind: IloKind::SignedIn { .. },
                ..
            })
        )
    });
    assert!(!filled, "{:?}", talk.lines);
}

/// A read whose page's one look found it showing what a password field held (the core's freeze)
/// comes back with no text and the tab withheld: the agent hears the core's refusal, as it does on
/// every later call, and a tab without its page still says that instead.
#[gpui::test]
fn a_read_the_pages_look_withholds_answers_the_cores_refusal(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    let (tab, request) = *browser
        .reads
        .borrow()
        .last()
        .expect("its text asked of the core");
    assert_eq!(tab, 20);
    browser.withheld.borrow_mut().insert(20);
    agents.update(cx, |agents, cx| agents.text_read(request, None, cx));
    let refused = answer.recv().unwrap();
    assert!(refused.error);
    assert_eq!(refused.text, WITHHELD_REFUSAL);
    // The call's one entry is the refusal.
    let reads: Vec<Activity> = agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.action == "read_page")
            .cloned()
            .collect()
    });
    assert_eq!(reads.len(), 1, "{reads:?}");
    assert_eq!(reads[0].refused.as_ref(), WITHHELD_REFUSAL);

    browser.withheld.borrow_mut().remove(&20);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    let (_, request) = *browser
        .reads
        .borrow()
        .last()
        .expect("its text asked of the core");
    agents.update(cx, |agents, cx| agents.text_read(request, None, cx));
    let none = answer.recv().unwrap();
    assert!(none.error);
    assert_ne!(none.text, WITHHELD_REFUSAL);
}

/// A read whose tab the user took while the page's one look was out comes back with no text: the
/// agent hears what any call on a held tab hears, and the log's one entry is that refusal.
#[gpui::test]
fn a_read_of_a_tab_the_user_took_meanwhile_answers_as_a_held_tabs_call(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let answer = call(
        &agents,
        "claude-code",
        "read_page",
        json!({ "tab": 20 }),
        cx,
    );
    let (_, request) = *browser
        .reads
        .borrow()
        .last()
        .expect("its text asked of the core");
    browser.held.borrow_mut().insert(20);
    agents.update(cx, |agents, cx| agents.text_read(request, None, cx));
    let refused = answer.recv().unwrap();
    assert!(refused.error);
    assert_eq!(refused.text, HELD);
    let reads: Vec<Activity> = agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.action == "read_page")
            .cloned()
            .collect()
    });
    assert_eq!(reads.len(), 1, "{reads:?}");
    assert_eq!(reads[0].refused.as_ref(), HELD);
}

/// The activity log and the 30-day record name the site a login was filled at, never its account.
#[gpui::test]
fn the_log_and_the_record_name_the_site_never_the_account(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    hello(&agents, cx);
    let answer = sign_in(&agents, 20, cx);
    agents.update(cx, |agents, cx| {
        agents.signed_in("claude-code", 20, "example.com", ACCOUNT.into(), cx)
    });
    done(&agents, asked(&browser), true, SIGNED, None, cx);
    answer.recv().unwrap();
    let filled = |steps: Vec<(String, String)>| {
        let found = steps
            .into_iter()
            .find(|(action, _)| action == "filled_login");
        found.map(|(_, site)| site)
    };
    let logged = (log(&agents, "claude-code", cx).into_iter())
        .map(|(action, site, _)| (action, site))
        .collect();
    assert_eq!(filled(logged).as_deref(), Some("example.com"));
    let activity: Vec<Activity> = agents.read_with(cx, |agents, _| {
        agents.activity("claude-code").cloned().collect()
    });
    assert!(!format!("{activity:?}").contains(ACCOUNT), "{activity:?}");

    bye(&agents, cx);
    let kept = agents.update(cx, |agents, cx| agents.take_ended(cx));
    let [(_, past)] = kept.as_slice() else {
        panic!("one record: {kept:?}");
    };
    let steps = (past.steps.iter())
        .map(|step| (step.action.to_string(), step.site.to_string()))
        .collect();
    assert_eq!(filled(steps).as_deref(), Some("example.com"));
    assert!(!format!("{past:?}").contains(ACCOUNT), "{past:?}");
}

/// The account is for the user's line to draw and formats nowhere else: whatever logs, panics
/// with or traces a line, its kind or the account itself prints no name, and it has no Display to
/// print it with.
#[test]
fn the_account_formats_nowhere_but_the_users_line() {
    let account = Account::new(ACCOUNT);
    let kind = IloKind::SignedIn {
        tab: 20,
        site: "example.com".into(),
        with: SignedInWith::Saved(account.clone()),
    };
    let line = IloLine {
        key: 1,
        kind: kind.clone(),
        settled: false,
    };
    for formatted in [
        format!("{account:?}"),
        format!("{kind:?}"),
        format!("{line:?}"),
    ] {
        assert!(!formatted.contains(ACCOUNT), "{formatted}");
    }
    assert_eq!(account.drawn(), ACCOUNT);
}

/// The switch is off until the user sets it, in every build: a profile that never set it reads
/// off, and so does one whose file holds the "sign_in" every save once wrote, which no save
/// writes again.
#[gpui::test]
fn signing_tasks_in_is_off_until_the_user_sets_it(cx: &mut TestAppContext) {
    let (agents, _, _) = super::connected(cx);
    assert!(!agents.read_with(cx, |agents, _| agents.sign_in_on()));
    agents.update(cx, |agents, cx| agents.load(r#"{"sign_in":true}"#, cx));
    assert!(!agents.read_with(cx, |agents, _| agents.sign_in_on()));
    let refused = sign_in(&agents, 20, cx).recv().unwrap();
    assert!(refused.error);
}

/// Set, on or off, the switch survives a save and the next launch, and the save drops the old key.
#[gpui::test]
fn the_users_setting_survives_a_save_and_a_relaunch(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("sign-in-switch-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agents.json");
    std::fs::write(&path, r#"{"sign_in":true}"#).unwrap();
    let launch = |cx: &mut TestAppContext| {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, Some(path.clone()), cx);
        let saved = std::fs::read_to_string(&path).unwrap_or_default();
        agents.update(cx, |agents, cx| agents.load(&saved, cx));
        agents
    };
    for on in [true, false] {
        let agents = launch(cx);
        agents.update(cx, |agents, cx| agents.set_sign_in_on(on, cx));
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.get("sign_in"), None, "{saved}");
        assert_eq!(
            launch(cx).read_with(cx, |agents, _| agents.sign_in_on()),
            on
        );
    }
    _ = std::fs::remove_dir_all(&dir);
}

/// Only the user turns signing tasks in with saved logins on or off: outside tests, Settings is the
/// one caller of set_sign_in_on, the switch is written only there and where the saved settings are
/// read at launch, and no tool's name or description names it, so no handler can be asked for it.
#[test]
fn only_settings_turns_signing_tasks_in_on_or_off() {
    fn sources(dir: &std::path::Path, found: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                if name != "tests" {
                    sources(&path, found);
                }
            } else if name.ends_with(".rs") && name != "tests.rs" {
                let source = std::fs::read_to_string(&path).unwrap();
                found.push((name, source));
            }
        }
    }
    let mut found = Vec::new();
    sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    let mut callers = Vec::new();
    let mut writers = Vec::new();
    for (file, source) in &found {
        let calls = source.matches("set_sign_in_on(").count();
        let defined = source.matches("fn set_sign_in_on(").count();
        callers.extend(std::iter::repeat_n(file.clone(), calls - defined));
        let writes = source.matches("self.sign_in_chosen = ").count();
        writers.extend(std::iter::repeat_n(file.clone(), writes));
    }
    assert_eq!(callers, ["settings.rs"]);
    writers.sort();
    assert_eq!(writers, ["agents.rs", "signing.rs"]);

    for agent in ["claude-code", "codex", "task-1"] {
        let listed = tools::list(agent).to_string().to_lowercase();
        for word in [
            "setting",
            "sign_in_on",
            "sign_in_chosen",
            "sign tasks in",
            "switch",
        ] {
            assert!(!listed.contains(word), "{agent}'s tools say {word:?}");
        }
        for name in tools::names(agent) {
            assert!(
                !(name.contains("setting") || name.contains("switch")),
                "{agent} has {name}"
            );
        }
    }
}
