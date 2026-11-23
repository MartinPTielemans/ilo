//! Saved reads (#3822) as Claude Code proposes, the user saves and agents use them: its tab 20 is
//! on www.example.com in its Space 2, of profile 1, and the core's reads are answered here.

use super::*;
use crate::agents::saved_read::Answer as CoreAnswer;

const PAGE: &str = "<table><tr><th>Name</th><th>City</th></tr>\
    <tr><td>Torre A</td><td>Lisboa</td></tr>\
    <tr><td>Torre B</td><td>Porto</td></tr>\
    <tr><td>Torre C</td><td>Faro</td></tr>\
    <tr><td>Torre D</td><td>Braga</td></tr></table>";

fn proposal() -> Value {
    json!({
        "name": "city buildings",
        "kind": "table",
        "url": "/buildings",
        "columns": [
            { "name": "Name", "type": "text", "from": "Name" },
            { "name": "City", "type": "text", "from": "City" },
        ],
        "tab": 20,
    })
}

fn page(url: &str, body: &str) -> CoreAnswer {
    CoreAnswer {
        status: 200,
        url: url.into(),
        kind: "text/html".into(),
        body: body.as_bytes().to_vec(),
        ..Default::default()
    }
}

/// Answers the core's last read with `answer`, as profile_read_done would.
fn answer_last(
    agents: &Entity<Agents>,
    browser: &Made,
    answer: CoreAnswer,
    cx: &mut TestAppContext,
) {
    let read = browser
        .saved_reads
        .borrow()
        .last()
        .expect("nothing was read")
        .1;
    agents.update(cx, |agents, cx| agents.read_done(read, answer, cx));
    cx.run_until_parked();
}

/// Claude Code's proposal of the table, read once and on its card.
fn proposed(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>, mpsc::Receiver<ToolResult>) {
    let (agents, browser, _) = connected(cx);
    let answer = call(&agents, "claude-code", "propose_read", proposal(), cx);
    assert_eq!(
        browser.saved_reads.borrow()[0].2,
        "https://www.example.com/buildings"
    );
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    (agents, browser, answer)
}

fn titles(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Vec<(String, Option<String>)> {
    agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .map(|entry| {
                let did = tools::Did {
                    tool: entry.action.to_string(),
                    site: entry.site.to_string(),
                    target: entry.target.to_string(),
                    ..Default::default()
                };
                (
                    tools::step_title(&did, tools::Tense::Done),
                    tools::read_detail(&entry.action, &entry.site, &entry.target),
                )
            })
            .collect()
    })
}

fn question(browser: &Made) -> Question {
    browser.asked.borrow().clone().expect("no card asks")
}

/// Lib test 1: the card shows the read from what reading it once gave, its name in the read block
/// and never in the question line; Save Read keeps it in the profile's file, and the log says so.
#[gpui::test]
fn save_read_keeps_the_proposal_and_logs_it(cx: &mut TestAppContext) {
    let (agents, browser, answer) = proposed(cx);
    let asked = question(&browser);
    let read = asked.read.clone().expect("the card has no read");
    assert_eq!(read.read.name, "city buildings");
    assert_eq!(read.rows.len(), 4);
    assert_eq!(read.rows[0].0, ["Torre A", "Lisboa"]);
    assert!(!asked.what.contains("city buildings"), "{}", asked.what);
    assert_eq!(asked.what.as_ref(), "save a read of www.example.com");
    // Not an action on the tab's page, so no "On <site>" head: its sentence stays whole.
    assert_eq!(asked.asks.as_ref(), "");
    assert!(
        answer.try_recv().is_err(),
        "it answered before the user did"
    );

    agents.update(cx, |agents, cx| agents.answer(asked.id, Answer::Once, cx));
    let saved = answer.try_recv().expect("Save Read didn't answer");
    assert!(!saved.error, "{}", saved.text);
    assert!(saved.text.starts_with("saved:"), "{}", saved.text);
    assert_eq!(*browser.asked.borrow(), None);
    let file = browser
        .reads_files
        .borrow()
        .get(&1)
        .cloned()
        .unwrap_or_default();
    let kept = crate::saved_reads::Store::read(&file);
    assert_eq!(kept.list().len(), 1);
    assert_eq!(kept.list()[0].asked_by, "Claude Code");
    // Never a row: the file keeps the definition and its state.
    assert!(
        !file.contains("Torre") && !file.contains("Lisboa"),
        "{file}"
    );
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Saved a read of www.example.com".to_string(),
            Some("city buildings · you said Save Read".to_string())
        )
    );
}

/// Lib test 1: Don't Save tells the agent it was declined, with the pass's words, keeps nothing,
/// and the next proposal asks afresh.
#[gpui::test]
fn dont_save_declines_and_keeps_nothing(cx: &mut TestAppContext) {
    let (agents, browser, answer) = proposed(cx);
    let id = question(&browser).id;
    agents.update(cx, |agents, cx| agents.answer(id, Answer::Dismiss, cx));
    let declined = answer.try_recv().expect("Don't Save didn't answer");
    assert!(declined.error);
    assert_eq!(
        declined.text,
        "declined: the user didn't save \"city buildings\". Do not propose it again now; read www.example.com in a tab instead."
    );
    assert!(
        browser
            .reads_files
            .borrow()
            .get(&1)
            .is_none_or(String::is_empty)
    );
    assert_eq!(
        titles(&agents, cx)[0].0,
        "You didn't save \"city buildings\""
    );

    let again = call(&agents, "claude-code", "propose_read", proposal(), cx);
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    assert!(again.try_recv().is_err());
    assert!(
        question(&browser).read.is_some(),
        "a second proposal didn't ask"
    );
}

/// Lib test 2: a card nobody answers is refused at ASK_WAIT through the questions' own path, with
/// the pass's words for the agent and the log.
#[gpui::test]
fn an_unanswered_proposal_is_declined_at_the_ask_wait(cx: &mut TestAppContext) {
    let (agents, browser, answer) = proposed(cx);
    cx.executor()
        .advance_clock(ASK_WAIT - Duration::from_secs(1));
    cx.run_until_parked();
    assert!(answer.try_recv().is_err(), "it gave up before ASK_WAIT");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let declined = answer.try_recv().expect("nothing answered at ASK_WAIT");
    assert!(declined.error);
    assert_eq!(
        declined.text,
        "declined: the request to save \"city buildings\" timed out with no answer. Do not propose it again now; read www.example.com in a tab instead."
    );
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Not answered: \"city buildings\"".to_string(),
            Some("www.example.com".to_string())
        )
    );
}

/// The agent going with a proposed read on its card and an upload waiting behind it leaves
/// neither: no card, no read saved, and the file field dropped, as the page gets nothing.
#[gpui::test]
fn the_agent_going_leaves_neither_its_proposal_nor_its_upload(cx: &mut TestAppContext) {
    let (agents, browser, proposing) = proposed(cx);
    let form = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Apply" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "button" }, "name": { "value": "Résumé" },
          "backendDOMNodeId": 61 },
    ]});
    let outline = call(&agents, "claude-code", "snapshot", json!({ "tab": 20 }), cx);
    assert!(!page_done(&agents, &browser, outline, form.to_string(), cx).error);
    let uploading = call(
        &agents,
        "claude-code",
        "upload",
        json!({ "tab": 20, "ref": "e1" }),
        cx,
    );
    let (_, request, action, _, _) = browser.pages.borrow().last().unwrap().clone();
    assert_eq!(action, PageAction::FileField);
    let field = json!({ "field": 7.0, "label": "Résumé", "site": "example.com" }).to_string();
    agents.update(cx, |agents, cx| agents.page_done(request, field, None, cx));
    cx.run_until_parked();
    assert_eq!(agents.read_with(cx, |agents, _| agents.asking.len()), 2);

    agents.update(cx, |agents, cx| {
        let bye = Event::Bye {
            agent: "claude-code".into(),
        };
        agents.handle(bye, cx)
    });
    cx.run_until_parked();
    assert!(agents.read_with(cx, |agents, _| agents.asking.is_empty()));
    assert_eq!(*browser.asked.borrow(), None);
    assert_eq!(*browser.dropped.borrow(), [7]);
    assert!(
        titles(&agents, cx)
            .iter()
            .all(|(title, _)| !title.starts_with("Saved"))
    );
    // Neither call is answered: the agent that made them is gone.
    assert!(proposing.try_recv().is_err() && uploading.try_recv().is_err());
}

/// Saves the proposal, as the user's Save Read does.
fn saved(cx: &mut TestAppContext) -> (Entity<Agents>, Rc<Made>) {
    let (agents, browser, answer) = proposed(cx);
    let id = question(&browser).id;
    agents.update(cx, |agents, cx| agents.answer(id, Answer::Once, cx));
    assert!(!answer.try_recv().unwrap().error);
    (agents, browser)
}

/// Lib test 8: the same name with other columns asks again, and Save logs what changed.
#[gpui::test]
fn a_changed_read_asks_again_and_logs_what_changed(cx: &mut TestAppContext) {
    let (agents, browser) = saved(cx);
    let same = call(&agents, "claude-code", "propose_read", proposal(), cx);
    let same = same.try_recv().expect("the same read asked again");
    assert!(same.text.starts_with("saved:"), "{}", same.text);

    let mut changed = proposal();
    changed["name"] = json!("City Buildings");
    changed["columns"] = json!([{ "name": "Name", "type": "text", "from": "Name" }]);
    let answer = call(&agents, "claude-code", "propose_read", changed, cx);
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    let asked = question(&browser);
    assert_eq!(asked.read.as_ref().unwrap().changes, ["columns"]);
    agents.update(cx, |agents, cx| agents.answer(asked.id, Answer::Once, cx));
    assert!(!answer.try_recv().unwrap().error);
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Changed a read of www.example.com".to_string(),
            Some("City Buildings: columns".to_string())
        )
    );
    let reads = agents.read_with(cx, |agents, _| agents.saved_reads(1));
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].read.columns.len(), 1);
}

/// Lib test 9: two uses of one read go one after the other, the second once the first answered;
/// two different reads go out together.
#[gpui::test]
fn uses_of_one_read_go_one_at_a_time(cx: &mut TestAppContext) {
    let (agents, browser) = saved(cx);
    let sent = || browser.saved_reads.borrow().len();
    let before = sent();
    let use_it = || json!({ "name": "city buildings" });
    let first = call(&agents, "claude-code", "use_read", use_it(), cx);
    let second = call(&agents, "claude-code", "use_read", use_it(), cx);
    assert_eq!(
        sent(),
        before + 1,
        "the second went before the first answered"
    );

    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    assert!(!first.try_recv().unwrap().error);
    assert_eq!(
        sent(),
        before + 2,
        "the second didn't go once the first answered"
    );
    assert!(second.try_recv().is_err());
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    assert!(!second.try_recv().unwrap().error);

    // Another read of its own goes beside one under way.
    let mut other = proposal();
    other["name"] = json!("towers");
    let answer = call(&agents, "claude-code", "propose_read", other, cx);
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    let id = question(&browser).id;
    agents.update(cx, |agents, cx| agents.answer(id, Answer::Once, cx));
    assert!(!answer.try_recv().unwrap().error);
    let before = sent();
    let _one = call(&agents, "claude-code", "use_read", use_it(), cx);
    let _two = call(
        &agents,
        "claude-code",
        "use_read",
        json!({ "name": "towers" }),
        cx,
    );
    assert_eq!(sent(), before + 2, "different reads waited on each other");
}

/// Lib test 10: use_read's answers by what the read came to: its rows, a sign-in with nothing of
/// where the site sent it, broken with its reason, and not_found. Each is logged with the read's
/// name and the address it read, which lib test 11's 30-day file keeps, and never a row.
#[gpui::test]
fn use_read_answers_by_what_the_read_came_to(cx: &mut TestAppContext) {
    let (agents, browser) = saved(cx);
    let use_it = || json!({ "name": "City buildings" });

    let rows = call(&agents, "claude-code", "use_read", use_it(), cx);
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    let rows = rows.try_recv().unwrap();
    assert!(!rows.error, "{}", rows.text);
    let rows: Value = serde_json::from_str(&rows.text).unwrap();
    assert_eq!(
        rows["rows"][1],
        json!({ "Name": "Torre B", "City": "Porto" })
    );
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Used saved read “city buildings”".to_string(),
            Some("www.example.com".to_string())
        )
    );

    let signed_out = call(&agents, "claude-code", "use_read", use_it(), cx);
    let redirect = "https://login.example.net/?next=%2Fbuildings&session=abc";
    answer_last(
        &agents,
        &browser,
        CoreAnswer {
            status: 302,
            url: "https://www.example.com/buildings".into(),
            redirect: Some(redirect.into()),
            ..Default::default()
        },
        cx,
    );
    let signed_out = signed_out.try_recv().unwrap();
    assert!(signed_out.error);
    assert!(
        signed_out
            .text
            .starts_with("needs_sign_in: www.example.com"),
        "{}",
        signed_out.text
    );
    assert!(!signed_out.text.contains("login.example.net"));

    let broken = call(&agents, "claude-code", "use_read", use_it(), cx);
    let changed = PAGE.replace("<th>Name</th>", "<th>Building</th>");
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", &changed),
        cx,
    );
    let broken = broken.try_recv().unwrap();
    assert!(broken.error);
    assert!(
        broken
            .text
            .starts_with("broken: www.example.com didn't match the saved read. "),
        "{}",
        broken.text
    );
    let reads = agents.read_with(cx, |agents, _| agents.saved_reads(1));
    assert!(matches!(
        reads[0].state,
        crate::saved_reads::State::Broken(_)
    ));

    // An error with part of a body is never rows, and says nothing of the read's state.
    let failed = call(&agents, "claude-code", "use_read", use_it(), cx);
    let mut partly = page("https://www.example.com/buildings", PAGE);
    partly.error = Some("ERR_CONNECTION_RESET".into());
    answer_last(&agents, &browser, partly, cx);
    let failed = failed.try_recv().unwrap();
    assert!(
        failed.error && failed.text.starts_with("unreachable:"),
        "{}",
        failed.text
    );
    let reads = agents.read_with(cx, |agents, _| agents.saved_reads(1));
    assert!(matches!(
        reads[0].state,
        crate::saved_reads::State::Broken(_)
    ));

    let missing = call(
        &agents,
        "claude-code",
        "use_read",
        json!({ "name": "nope" }),
        cx,
    );
    let missing = missing.try_recv().unwrap();
    assert!(missing.error);
    assert!(
        missing
            .text
            .starts_with("not_found: no saved read named \"nope\"")
    );
    assert_eq!(titles(&agents, cx)[0].0, "No saved read named \"nope\"");

    // Lib test 11: what the log, and so the 30-day file, keeps of each use.
    let entries = agents.read_with(cx, |agents, _| {
        (agents.activity("claude-code"))
            .filter(|entry| entry.action.contains("read"))
            .map(|entry| {
                format!(
                    "{} {} {} {} {} {}",
                    entry.action, entry.site, entry.target, entry.url, entry.refused, entry.said
                )
            })
            .collect::<Vec<_>>()
    });
    let used = entries
        .iter()
        .find(|entry| entry.starts_with("use_read "))
        .unwrap();
    assert_eq!(
        used.trim(),
        "use_read www.example.com city buildings https://www.example.com/buildings"
    );
    for entry in &entries {
        for text in ["Torre", "Lisboa", "Porto", "login.example.net", "abc"] {
            assert!(!entry.contains(text), "the log keeps {text}: {entry}");
        }
    }
}

/// Lib test 12: an agent whose Space is in another profile lists none of profile 1's reads, and a
/// profile with no folder, as a private window's, refuses a proposal and keeps no file.
#[gpui::test]
fn reads_are_the_profiles_and_a_private_one_keeps_none(cx: &mut TestAppContext) {
    let (agents, browser) = saved(cx);
    let listed = |cx: &mut TestAppContext| {
        let answer = call(&agents, "claude-code", "list_reads", json!({}), cx);
        let answer: Value = serde_json::from_str(&answer.try_recv().unwrap().text).unwrap();
        answer["reads"].as_array().unwrap().len()
    };
    assert_eq!(listed(cx), 1);
    browser.space_profiles.borrow_mut().insert(2, 7);
    assert_eq!(listed(cx), 0);

    browser.space_profiles.borrow_mut().insert(2, 8);
    browser.no_folder.borrow_mut().insert(8);
    let refused = call(&agents, "claude-code", "propose_read", proposal(), cx);
    let refused = refused.try_recv().expect("a private proposal waited");
    assert!(
        refused.error && refused.text.contains("private window"),
        "{}",
        refused.text
    );
    assert!(!browser.reads_files.borrow().contains_key(&8));
    assert!(agents.read_with(cx, |agents, _| !agents.keeps_reads(8)));
}

/// A proposal is of its tab's site alone: an address on another origin, the same host over http
/// too, is refused before anything is read, and a ⌘T task has none of the tools.
#[gpui::test]
fn a_proposal_reads_only_its_tabs_site(cx: &mut TestAppContext) {
    let (agents, browser, _) = connected(cx);
    let mut away = proposal();
    away["url"] = json!("http://www.example.com/buildings");
    let refused = call(&agents, "claude-code", "propose_read", away, cx);
    let refused = refused.try_recv().unwrap();
    assert!(refused.error && refused.text.contains("can't target another site or scheme"));
    assert!(
        browser.saved_reads.borrow().is_empty(),
        "it read another site"
    );
    assert!(*browser.asked.borrow() == None);
    for tool in ["propose_read", "use_read", "list_reads"] {
        assert!(tools::names("claude-code").contains(&tool.to_string()));
        assert!(!tools::names("task-1").contains(&tool.to_string()));
    }
}

/// Settings' Remove Read, with no dialog: ⌘Z brings it back where it was, and the log says it went.
#[gpui::test]
fn remove_read_and_undo_bring_it_back_in_place(cx: &mut TestAppContext) {
    let (agents, browser) = saved(cx);
    let mut other = proposal();
    other["name"] = json!("towers");
    let answer = call(&agents, "claude-code", "propose_read", other, cx);
    answer_last(
        &agents,
        &browser,
        page("https://www.example.com/buildings", PAGE),
        cx,
    );
    let id = question(&browser).id;
    agents.update(cx, |agents, cx| agents.answer(id, Answer::Once, cx));
    assert!(!answer.try_recv().unwrap().error);
    let names = |cx: &mut TestAppContext| {
        agents.read_with(cx, |agents, _| {
            (agents.saved_reads(1).into_iter())
                .map(|saved| saved.read.name)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(names(cx), ["city buildings", "towers"]);

    agents.update(cx, |agents, cx| agents.remove_read(1, "city buildings", cx));
    assert_eq!(names(cx), ["towers"]);
    let file = browser.reads_files.borrow().get(&1).cloned().unwrap();
    assert!(!file.contains("city buildings"));
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Removed a read of www.example.com".to_string(),
            Some("city buildings · by you".to_string())
        )
    );
    assert!(agents.update(cx, |agents, cx| agents.undo_remove_read(cx)));
    assert_eq!(names(cx), ["city buildings", "towers"]);
    assert!(!agents.update(cx, |agents, cx| agents.undo_remove_read(cx)));

    agents.update(cx, |agents, cx| agents.remove_read(1, "towers", cx));
    agents.update(cx, |agents, _| agents.forget_removed_reads());
    assert!(!agents.update(cx, |agents, cx| agents.undo_remove_read(cx)));
    let missing = call(
        &agents,
        "claude-code",
        "use_read",
        json!({ "name": "towers" }),
        cx,
    );
    assert!(missing.try_recv().unwrap().text.starts_with("not_found:"));
}

/// Lib test 7: with its site on Always ask first a saved read is paused: list_reads says so, a use
/// reads nothing and says why, and a new proposal there is refused with no card. Off the list,
/// the read reads again.
#[gpui::test]
fn always_ask_first_pauses_reads_and_refuses_proposals(cx: &mut TestAppContext) {
    let (agents, browser, answer) = proposed(cx);
    let asked = question(&browser);
    agents.update(cx, |agents, cx| agents.answer(asked.id, Answer::Once, cx));
    assert!(!answer.try_recv().expect("Save Read didn't answer").error);
    agents.update(cx, |agents, cx| {
        agents.fence("example.com", cx);
    });
    let state = |cx: &mut TestAppContext| {
        let listed = call(&agents, "claude-code", "list_reads", json!({}), cx)
            .try_recv()
            .expect("list_reads didn't answer");
        let listed: Value = serde_json::from_str(&listed.text).unwrap();
        listed["reads"][0]["state"]
            .as_str()
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(state(cx), "paused");

    let reads = browser.saved_reads.borrow().len();
    let used = call(
        &agents,
        "claude-code",
        "use_read",
        json!({ "name": "city buildings" }),
        cx,
    )
    .try_recv()
    .expect("a paused read didn't answer");
    assert!(used.error);
    assert_eq!(
        used.text,
        "paused: www.example.com is on Always ask first, so saved reads for it are stopped."
    );
    assert_eq!(
        browser.saved_reads.borrow().len(),
        reads,
        "a paused read read"
    );
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Paused: city buildings".to_string(),
            Some("www.example.com is on Always ask first".to_string())
        )
    );

    let mut again = proposal();
    again["name"] = json!("other buildings");
    let refused = call(&agents, "claude-code", "propose_read", again, cx)
        .try_recv()
        .expect("a proposal on a fenced site didn't answer");
    assert!(refused.error);
    assert_eq!(
        refused.text,
        "not_allowed: www.example.com is on Always ask first, so reads can't be saved for it. \
         Open www.example.com in a tab to ask the user instead."
    );
    assert_eq!(*browser.asked.borrow(), None, "a card asked");
    assert_eq!(
        browser.saved_reads.borrow().len(),
        reads,
        "the proposal read"
    );
    assert_eq!(
        titles(&agents, cx)[0],
        (
            "Can't save a read of www.example.com".to_string(),
            Some("On your Always ask first list".to_string())
        )
    );

    agents.update(cx, |agents, cx| agents.unfence("example.com", cx));
    assert_eq!(state(cx), "ready");
    let _used = call(
        &agents,
        "claude-code",
        "use_read",
        json!({ "name": "city buildings" }),
        cx,
    );
    assert_eq!(
        browser.saved_reads.borrow().len(),
        reads + 1,
        "off the list, the read didn't read"
    );
}
